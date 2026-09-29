# M6 — Tauri commands, events, watcher, and the typed client

Status: awaiting approval
Project: tarpack   Depends on: M4 (landed), M5 (landed)

## Goal

Expose the Tar Packager to the frontend through typed Tauri commands and
events, plus a typed TS client in `lib/`. After this task, the UI tasks can
build the whole tool without touching Rust.

## Context

What exists:

- **M1:** the Tauri shell `apps/desktop/src-tauri`, with the tool registry at
  `src/tools/mod.rs`. Tool commands are prefixed `<tool>_`. The `ts-rs` export
  goes to `apps/desktop/src/lib/generated/`, and `lib/tauri.ts` holds the
  wrappers. Capabilities are in `capabilities/default.json`.
- **M2:** `fm_core::{AppDirs, Store, FmError}`
- **M3:** `fm_tarpack::manifest::{load, LoadedManifest, Diagnostic, ManifestView}`.
  `LoadedManifest.sha256` hashes the exact file bytes. The format is in
  `docs/tarpack-manifest.md`.
- **M3:** `fm_tarpack::format::ArchiveFormat { Tar, TarGz, TarZst, TarXz }`
  with `ALL`, `extension()`, `from_file_name()` (recognises `.tar`,
  `.tar.gz`, `.tgz`, `.tar.zst`, and `.tar.xz`, case-insensitively), and
  `with_extension()`. `Manifest::default_format()` exists too.
- **M4:** `fm_tarpack::archive::{ArchivePlan, write_archive, BuildSummary, BuildError, Progress, BuildPhase}`.
  `write_archive(plan, out_path, format, overwrite, progress)`. `BuildSummary`
  carries `format`, `bytes` (on disk), `uncompressed_bytes`, `sha256_hex`,
  `extract_command`, and `normalized_entries`. `Progress` carries
  `phase: Writing | Verifying`, `entry_id: Option`, `bytes_done`, and
  `bytes_total`, in uncompressed bytes. `BuildError::SourceChanged { id }`
  exists.
- **M5:** `fm_tarpack::sources::{Assignments, EntryStatus, match_dropped, apply, DropOutcome, RememberedState}`.
  `RememberedState` has
  `remember(manifest_path, &Assignments, output, format)`, `restore`,
  `restore_output`, `restore_format` (remembered, else the manifest default),
  and `touch_recent`.

**The boundary.** You own all Rust and `apps/desktop/src/lib/`. You do not
create components, views, or styles; the `ui-implementer` builds those from
this API.

**Rules that bind this task.**

- A single source of truth. Session state lives in Rust as Tauri-managed state,
  and every command returns a full `TarpackSession` snapshot, which the UI
  renders. There is no hand-written TS duplicate of any Rust type; everything
  goes through `lib/generated/`.
- The UI must never build an archive the user did not see. `tarpack_build`
  re-reads the manifest file, and if its hash differs from the loaded one it
  fails with `ManifestChangedOnDisk`, writing nothing.
- An existing output file is overwritten only when the UI passes
  `overwrite: true`, after the user confirms.
- **The backend owns format and extension logic** (decided). The UI gets the
  format list, the extensions, the suggested file name, and the extraction
  command from the contract, and computes none of them itself.
- The archive stores absolute names, and the target extracts with GNU tar `-P`.
  This task passes `extract_command` through to the UI, and does not build
  commands itself.
- Capabilities grant only what is used.
- Tests never touch real user files or the real app-data directory. Test the
  command logic as plain functions over `AppDirs::at(tempdir)`; the Tauri
  handlers are thin glue.

## Files

- `apps/desktop/src-tauri/src/tools/{mod.rs, tarpack.rs}`: `tarpack.rs` may be
  a folder module if it grows
- `apps/desktop/src-tauri/src/lib.rs`: register through the tool registry
- `apps/desktop/src-tauri/capabilities/default.json`
- `apps/desktop/src-tauri/Cargo.toml`: adds `notify` (or
  `notify-debouncer-mini`) and depends on `fm-core` and `fm-tarpack`
- `apps/desktop/src/lib/tarpack.ts`, `apps/desktop/src/lib/tauri.ts`, and
  their tests
- Regenerated `apps/desktop/src/lib/generated/*`

## Contract

**`TarpackSession`** (Rust, exported to TS):

```
{
  manifest: null | {
    path, name, outputName, hash,
    entries: [{ id, source, targetPath, mode, modeText, owner,
                normalizeEol: boolean,
                assigned: string|null, status: "ready"|"missing"|"unassigned" }],
    errors: Diagnostic[], warnings: Diagnostic[]
  },
  outputPath: string | null,         // always ends with formats[format].extension
  format: ArchiveFormat,             // "tar" | "tarGz" | "tarZst" | "tarXz"
  formats: ArchiveFormatOption[],    // [{ format, extension }] in ArchiveFormat::ALL order
  suggestedOutputName: string | null,// file name for the Save dialog, current extension
  readyCount, totalCount,
  canBuild: boolean,
  buildBlockedReason: null | "noManifest" | "manifestInvalid" | "entriesNotReady" | "noOutput",
  stateWarning: string | null        // from fm-core StoreWarning
}
```

`targetPath` is the absolute stored name (`/opt/...`).

**Format and output-name rules** (decided):

- **On open** (and on the first `tarpack_session()` restore), `format` is
  `RememberedState::restore_format(...)`: the last format chosen for this
  manifest, else the `output_name` suffix, else `tar`. `outputPath` is the
  remembered last output, normalised to that format's extension.
- `suggestedOutputName` is `format.with_extension(output_name)` when the
  manifest has an `output_name`, else `format.with_extension(<manifest file stem>)`.
  It is `null` with no manifest.
- `tarpack_set_format(format)` sets the format. If `outputPath` is set, it
  rewrites the path's file name with `format.with_extension(...)`. It persists
  the choice, as `last_format`, for this manifest.
- `tarpack_set_output(path)`:
  - If the file name ends with the current format's extension, it is kept.
  - If it ends with a *different* known archive suffix, `format` switches to
    that format and is persisted, because the user typed that name.
  - Otherwise the current extension is appended.
- Work on `OsStr`/`PathBuf`; never lossily convert the path, except for the
  display strings in the session.

When a manifest fails validation, `manifest` is still present, with its `path`,
its `errors`, and whatever entries were readable, so that the UI can show the
errors.

**Commands.** Each returns `Result<T, TarpackError>`, where `TarpackError` is
`{ kind, message, entryId? }` and exported to TS.

| Command | Returns | Notes |
| --- | --- | --- |
| `tarpack_session()` | `TarpackSession` | On first call, restore the most recent manifest and its remembered sources. |
| `tarpack_open_manifest(path)` | `TarpackSession` | Load, restore remembered sources, `touch_recent`, start watching. |
| `tarpack_reload_manifest()` | `TarpackSession` | Keep assignments by id. |
| `tarpack_assign_dropped(paths)` | `{ session, outcome: DropOutcome }` | |
| `tarpack_assign(id, path)` | `TarpackSession` | |
| `tarpack_clear(id)` | `TarpackSession` | Clears the assignment only. |
| `tarpack_set_output(path)` | `TarpackSession` | Extension normalisation and format switch, as above. |
| `tarpack_set_format(format)` | `TarpackSession` | Rewrites the `outputPath` extension and remembers the format per manifest. |
| `tarpack_build(overwrite)` | `BuildSummary` | Runs on `spawn_blocking` with the session's `format`. Errors: `OutputExists`, `ManifestChangedOnDisk`, `SourceMissing`, `SourceUnreadable`, `SourceChanged`, `VerifyFailed`, … |
| `tarpack_recent_manifests()` | `string[]` | |
| `tarpack_open_in_editor()` | `()` | Opener plugin, on the manifest path. |
| `tarpack_reveal_output()` | `()` | Opener plugin; reveal the output in Explorer. |
| `tarpack_create_manifest_from_example(path)` | `TarpackSession` | Writes the bundled `examples/tarpack/example.toml` (`include_str!`). Errors if the path exists. |

Every mutating command persists `RememberedState` afterwards, including the
current format.

**`BuildSummary`** as the UI sees it (generated from M4's type, camelCase):
`{ path, format, entries, files, dirs, bytes, uncompressedBytes, sha256Hex, extractCommand, normalizedEntries: [{ id, crlfReplaced }] }`.

**Events.**

- `tarpack://manifest-changed` carries `{ path }`. It is sent by a watcher on
  the loaded manifest file, debounced at 300 ms, and re-armed on open. A new
  open or reload replaces the watcher.
- `tarpack://build-progress` carries
  `{ phase: "writing" | "verifying", entryId: string | null, bytesDone, bytesTotal }`.
  The byte counts are uncompressed tar-stream bytes. The verifying phase
  follows the writing phase and has its own 0-to-total run.

**`lib/tauri.ts`** adds typed wrappers:

- `openFileDialog({ filters?, defaultPath? })` and
  `saveFileDialog({ defaultPath, filters? })`, over the dialog plugin.
  `filters` is `{ name, extensions }[]`, as the dialog plugin takes it. Note
  that the plugin's `extensions` are suffixes without the leading dot, and
  Windows matches only the last one (for example `zst`). The backend's
  normalisation in `tarpack_set_output` is what guarantees the full extension.
- `onDragDrop(handler)`, over `getCurrentWebview().onDragDropEvent`, which
  yields `{ type: "enter"|"over"|"leave"|"drop", paths }`
- `listen` and `invoke`, typed over the generated types

**`lib/tarpack.ts`** exports one async function per command (including
`setFormat(format)`), `onManifestChanged`,
and `onBuildProgress`, all typed from `lib/generated/`.

## Acceptance criteria

- Every command and event is typed end to end. `npm run typecheck` fails if a
  Rust type changes without regenerating.
- `canBuild` is true only when a manifest is loaded with no errors, every entry
  is `ready`, and `outputPath` is set. `buildBlockedReason` names the first
  failing condition, in the order listed in the contract.
- An external edit to the manifest produces exactly one `manifest-changed`
  event.
- A build after an unseen edit fails with `ManifestChangedOnDisk` and writes
  nothing.
- The output path's extension always matches `format`, after open,
  `set_format`, and `set_output`. The format is restored per manifest across a
  simulated restart (new state over the same temp app dir).
- A build writes the session's format, and the returned summary carries it and
  the extraction command.
- Capabilities list only the dialog, opener, and event permissions used.

## Tests proving completion

- `cargo test -p filemanager tools::tarpack`, over the plain-function core with
  `AppDirs::at(TempDir)` and fixture files in the temp dir:
  - `session_restores_recent_manifest`
  - `open_restores_remembered_sources`
  - `reload_keeps_assignments_by_id`
  - `can_build_reasons_in_order`
  - `build_refuses_when_manifest_changed`
  - `build_refuses_existing_output_without_overwrite`
  - `create_from_example_refuses_existing_path`
  - `set_format_rewrites_output_extension`: `.tar` to `.tar.zst` to
    `.tar.xz`, a `.tgz` path, and a path with no suffix
  - `set_output_appends_or_switches_format`
  - `format_restored_per_manifest`
  - `suggested_output_name_follows_format`
  - `build_uses_session_format`: build a `.tar.zst`, and assert that the file
    starts with the zstd magic `28 B5 2F FD` and that the summary's `format`
    and `extractCommand` agree
  - `watcher_emits_once_per_edit`: allowed to be `#[ignore]` on CI if
    file-watch timing is flaky there; must run locally
- `npm run test`: `lib/tarpack.test.ts` and `lib/tauri.test.ts`, with
  `@tauri-apps/api` mocked, checking that each wrapper calls the right command
  with the right arguments, including `setFormat` and `saveFileDialog` with
  `filters`.
- `generated_types_are_current`
- The CI shell build on Windows.

## Out of scope

- Any component, view, style, or design decision.
- Packaging (M7).

## Risks

- The Linux CI job does not build the shell. The `tools::tarpack` tests must
  therefore live where Linux CI can run them, or the Windows job must run them.
  Put the plain-function core in a module that does not need the Tauri runtime,
  and make sure one CI job actually runs these tests.
