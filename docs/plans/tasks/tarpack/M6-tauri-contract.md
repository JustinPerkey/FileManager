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
  `src/tools/mod.rs`. Tool commands are prefixed `<tool>_`. `lib/tauri.ts`
  holds the wrappers. Capabilities are in `capabilities/default.json`. The
  CSP is in `tauri.conf.json`: IPC is allowed via `connect-src ipc:
  http://ipc.localhost`, and nothing remote is allowed. This task needs no
  CSP change; do not loosen it.
- **M1, registry rule.** Tauri's `Builder::invoke_handler` and
  `Builder::setup` each replace any earlier call. So:
  - `tools::register` in `src/tools/mod.rs` is the only place that calls
    them, once each;
  - `src/lib.rs` already calls `tools::register(builder)` and must not
    change;
  - the test `tools::tests::builder_hooks_only_in_tool_registry` enforces the
    rule. It scans the code lines of every `.rs` file under `src/` for
    `.invoke_handler(` and `.setup(`. Lines whose trimmed start is `//`
    (`//`, `///`, `//!`) are skipped in every file, so doc comments may name
    these methods freely. In `tools/mod.rs` it counts at most one code line
    with each, before `#[cfg(test)]`. Any other file with a code line
    containing either fails. The `//!` example at the top of `tools/mod.rs` is
    not counted, so your real calls there make each count exactly 1.
- **M1, generated types.** The ts-rs export is the `#[cfg(test)]` module
  `apps/desktop/src-tauri/src/generated_types.rs`, in this crate:
  - its `EXPORTERS` list already holds the `fm_tarpack` types from M3–M5;
  - you append this task's root types to it;
  - regenerate with
    `UPDATE_GENERATED=1 cargo test -p filemanager --lib generated_types_are_current`
    (the PowerShell form is in `CLAUDE.md`);
  - `#[ts(export)]` is not used;
  - `ts-rs` is already a `[dependencies]` entry of this crate
    (`{ workspace = true }`).
- **M1, CI.** Both CI jobs run `cargo test --workspace`. Linux installs the
  webkit2gtk dev packages to build this crate. Every test in this crate runs
  on both platforms.
- **M2:** `fm_core::{AppDirs, Store, FmError}`
- **M3:** `fm_tarpack::manifest::{load, LoadedManifest, Diagnostic, ManifestView}`.
  `LoadedManifest.sha256` hashes the exact file bytes. The format is in
  `docs/tarpack-manifest.md`.
- **M3:** `fm_tarpack::format::ArchiveFormat { Tar, TarGz, TarZst, TarXz }`
  with `ALL`, `extension()`, `from_file_name()` (recognises `.tar`,
  `.tar.gz`, `.tgz`, `.tar.zst`, and `.tar.xz`, case-insensitively), and
  `with_extension()`, and `filter_extension()` (`"tar"`, `"gz"`, `"zst"`,
  or `"xz"`: the last suffix without its dot, as a Save-dialog filter needs).
  `Manifest::default_format()` exists too.
- **M4:** `fm_tarpack::archive::{ArchivePlan, write_archive, BuildSummary, BuildError, Progress, BuildPhase}`.
  `write_archive(plan, out_path, format, overwrite, progress)`. `BuildSummary`
  carries `format`, `bytes` (on disk), `uncompressed_bytes`, `sha256_hex`,
  `extract_command`, and `normalized_entries`. `Progress` carries
  `phase: Writing | Verifying`, `entry_id: Option`, `bytes_done`, and
  `bytes_total`, in uncompressed bytes. Each phase ends with an event where
  `bytes_done == bytes_total` and `entry_id` is `None`. After that last
  verifying event only a rename remains, because the SHA-256 is computed during
  verification. `BuildError::SourceChanged { id }` and `VerifyFailed` exist. On
  any error, nothing is saved at the output path, and a pre-existing file there
  is left unchanged. M4's u64 fields are already annotated to generate as TS
  `number`.
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
- **No `bigint` crosses the boundary.** ts-rs generates `u64`/`i64` as `bigint`,
  which the UI cannot format or compare as it does numbers. Every 64-bit
  integer field in a type exported by this task carries `#[ts(type = "number")]`.
  Session counts are `u32`, or annotated the same way. All such values stay far
  below 2^53. A test enforces this across the whole generated directory.
- Tests never touch real user files or the real app-data directory. Test the
  command logic as plain functions over `AppDirs::at(tempdir)`; the Tauri
  handlers are thin glue.

## Files

- `apps/desktop/src-tauri/src/tools/tarpack.rs` (a folder module
  `tools/tarpack/` if it grows). It contributes only:
  - the `#[tauri::command]` functions, all named `tarpack_*`;
  - a constructor for the managed `TarpackState`;
  - if the watcher needs startup work, `pub(super) fn setup(app: &mut tauri::App)`.

  It never receives or returns the `Builder`. Emitting events from the watcher
  uses an `AppHandle` taken from the command that opens or reloads the
  manifest, or from `setup`.
- `apps/desktop/src-tauri/src/tools/mod.rs`: `register` gains:
  - `.manage(tarpack::...)`;
  - the crate's **single** `.invoke_handler(tauri::generate_handler![...])`,
    listing every `tarpack_*` command;
  - if needed, the single `.setup(...)` calling `tarpack::setup`.

  Write each call as real code, chained on `builder`. Update the `//!` doc
  example if the shape changes; it is a comment, so the hook test ignores it.
  `src/lib.rs` is not edited.
- `apps/desktop/src-tauri/src/generated_types.rs`:
  - append the exporters for `TarpackSession`, `ArchiveFormatOption`,
    `TarpackError`, `TarpackErrorKind`, and the event payload types;
  - add the two tests below (`generated_types_have_no_bigint`,
    `error_kind_is_string_union`) next to `generated_types_are_current`.
- `apps/desktop/src-tauri/capabilities/default.json`
- `apps/desktop/src-tauri/Cargo.toml`: adds `notify` (or
  `notify-debouncer-mini`). It already depends on `fm-core`, `fm-tarpack`,
  and `ts-rs` from M1.
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
  outputPath: string | null,         // always ends with the extension of the `formats` entry whose `format` equals `format`
  format: ArchiveFormat,             // "tar" | "tarGz" | "tarZst" | "tarXz"; "tar" when manifest is null
  formats: ArchiveFormatOption[],    // [{ format, extension, filterExtension }] in ArchiveFormat::ALL order; always all four
  suggestedOutputName: string | null,// file name for the Save dialog, current extension
  readyCount, totalCount,
  canBuild: boolean,
  buildBlockedReason: null | "noManifest" | "manifestInvalid" | "entriesNotReady" | "noOutput",
  stateWarning: string | null        // from fm-core StoreWarning
}
```

`targetPath` is the absolute stored name (`/opt/...`).

**`ArchiveFormatOption`** (Rust, exported to TS):

```
{ format: ArchiveFormat, extension: string, filterExtension: string }
```

For example, `{ format: "tarZst", extension: ".tar.zst", filterExtension: "zst" }`.
`filterExtension` comes from `ArchiveFormat::filter_extension()`. It is what
the UI passes in a Save-dialog filter's `extensions`, so the UI never derives
it from `extension`.

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
- **With no manifest loaded:**
  - `format` is `"tar"`;
  - `formats` still lists all four;
  - `outputPath` and `suggestedOutputName` are `null`.
  - `tarpack_set_format` and `tarpack_set_output` fail with `NoManifest` and
    change nothing. Format and output are remembered per manifest, so without
    one there is nowhere to keep them. The UI disables both controls in this
    state.
  - When a manifest is opened, `format` and `outputPath` come from that
    manifest's remembered state, as above.

When a manifest fails validation, `manifest` is still present, with its `path`,
its `errors`, and whatever entries were readable, so that the UI can show the
errors.

**Commands.** Each returns `Result<T, TarpackError>`, where `TarpackError` is
`{ kind: TarpackErrorKind, message: string, entryId?: string }` and exported to
TS. `entryId` uses `#[serde(skip_serializing_if = "Option::is_none")]` and
`#[ts(optional)]`, so the generated type really is `entryId?: string`.

**`TarpackErrorKind` is a closed set.** It is a fieldless Rust enum in
`src-tauri/src/tools/tarpack` (next to `TarpackError`), deriving `Serialize`
and `ts_rs::TS`, with variant names serialised unchanged (PascalCase). ts-rs
then generates a string-literal union:
`type TarpackErrorKind = "NoManifest" | "ManifestUnreadable" | …`. The UI
switches over it exhaustively, so:

- The variants are exactly the ones below. Every error path maps to one of
  them, and there is no catch-all string.
- Adding, removing, or renaming a variant is a UI contract change. The UI
  mapping must change in the same pipeline.

| Kind | Raised by | Meaning | `entryId` |
| --- | --- | --- | --- |
| `NoManifest` | any command needing a loaded manifest | No manifest is loaded | — |
| `ManifestUnreadable` | open, reload | The manifest file could not be read (missing, permission) | — |
| `ManifestInvalid` | build | The manifest has errors (defensive: `canBuild` already blocks this) | — |
| `ManifestChangedOnDisk` | build | The file hash differs from the loaded one | — |
| `UnknownEntry` | assign, clear | No entry has that id | the id |
| `NotAFile` | assign | The path is not an existing regular file | the id |
| `NoOutput` | build | No output path is set (defensive) | — |
| `EntriesNotReady` | build | An entry is unassigned or missing (defensive; from `PlanError::Unassigned`) | the first id |
| `OutputExists` | build | The output exists and `overwrite` was false | — |
| `PathExists` | create from example | The target path already exists | — |
| `SourceMissing` | build | A source vanished | the id |
| `SourceUnreadable` | build | A source could not be read | the id |
| `SourceChanged` | build | A source's size changed during the build | the id |
| `VerifyFailed` | build | The written archive failed verification; nothing saved | — |
| `BuildInProgress` | build | A build is already running | — |
| `OpenerFailed` | open in editor, reveal output | The opener plugin failed | — |
| `Io` | any | Any other I/O failure; `message` carries the detail | when known |

Map M4's `BuildError` and M5/M2 errors onto these kinds in one `From` impl or
match, so the mapping is reviewable in one place.

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
  `{ phase: "writing" | "verifying", entryId: string | null, bytesDone: number, bytesTotal: number }`
  (u64 on the Rust side, annotated `#[ts(type = "number")]`).
  - The byte counts are uncompressed tar-stream bytes. Both phases use the same
    `bytesTotal`.
  - The verifying phase follows the writing phase and has its own run from 0
    to the total.
  - **`entryId`** in both phases is the id of the file entry whose bytes are
    being written or checked. It is `null` while directory records or long-name
    records are processed, and on each phase's final event.
  - **Final event.** Each phase ends with exactly one event where
    `bytesDone == bytesTotal` and `entryId` is `null`.
  - **No event follows the final verifying event.** The only work left is an
    fsync and a rename; the SHA-256 is computed during verification. The
    `tarpack_build` promise then resolves or rejects. There is no
    `finalizing` phase. The UI may show a "Finishing…" state between the final
    verifying event and resolution.
  - A failed build stops emitting events at the point of failure.

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

**Clipboard: not wrapped.** `lib/tauri.ts` provides no clipboard function, and
this task adds no clipboard plugin or capability. The UI calls
`navigator.clipboard.writeText` directly. It is available in WebView2, because
the app origin (`tauri.localhost`) is a secure context and Copy runs from a
user click. This keeps capabilities minimal. If the M7 end-to-end check finds
it failing in the packaged exe, the fix is a new backend task adding
`tauri-plugin-clipboard-manager` with only `allow-write-text` and a
`writeClipboardText` wrapper; that would be a UI contract change.

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
- Capabilities list only the dialog, opener, and event permissions used. There
  is no clipboard permission.
- The generated `TarpackErrorKind.ts` is a string-literal union of exactly the
  kinds in the table, and `TarpackError.ts` has `kind: TarpackErrorKind` and
  `entryId?: string`.
- No file in `apps/desktop/src/lib/generated/` contains `bigint`.
- With no manifest, the session has `format: "tar"`, all four `formats`, and
  null output fields. `setFormat` and `setOutput` fail with `NoManifest` and
  change nothing.
- `formats` carries `filterExtension` for every format.
- A build's progress events follow the phase, `entryId`, and final-event rules
  above.

## Tests proving completion

- `cargo test -p filemanager tools::tarpack` (runs on both CI jobs), over the plain-function core with
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
  - `no_manifest_session_defaults_and_rejects_format_and_output`
  - `formats_carry_filter_extension`
  - `error_kinds_map_from_build_errors`: every `BuildError` variant and
    `PlanError` maps to its kind and `entryId`
  - `build_progress_events_follow_contract`: capture the events of a build
    with two files, and assert:
    - writing reaches `bytesDone == bytesTotal` with `entryId: null`;
    - verifying restarts from 0, carries file ids, and ends with one final
      null-id event;
    - nothing is emitted after it.
  - `build_uses_session_format`: build a `.tar.zst`, and assert that the file
    starts with the zstd magic `28 B5 2F FD` and that the summary's `format`
    and `extractCommand` agree
  - `watcher_emits_once_per_edit`: allowed to be `#[ignore]` on CI if
    file-watch timing is flaky there; must run locally
- `npm run test`: `lib/tarpack.test.ts` and `lib/tauri.test.ts`, with
  `@tauri-apps/api` mocked, checking that each wrapper calls the right command
  with the right arguments, including `setFormat` and `saveFileDialog` with
  `filters`.
- In `apps/desktop/src-tauri/src/generated_types.rs`, all run by
  `cargo test --workspace` on both CI jobs:
  - `generated_types_are_current`: check mode, after regenerating;
  - `generated_types_have_no_bigint`: scans `apps/desktop/src/lib/generated/`
    (read-only) and fails on any `bigint`;
  - `error_kind_is_string_union`: asserts that the generated
    `TarpackErrorKind.ts` is a union of string literals matching the table.
- `tools::tests::builder_hooks_only_in_tool_registry` still passes.
- The CI shell build on Windows.

## Out of scope

- Any component, view, style, or design decision.
- Packaging (M7).

## Risks

- Both CI jobs build the shell and run its tests (M1), so the `tools::tarpack`
  tests run on Linux and Windows. Keep the command logic as plain functions
  that do not need a running Tauri app, so they stay fast and deterministic.
  The handlers are thin glue.
- `generated_types_have_no_bigint` and `error_kind_is_string_union` read the
  committed files. Always regenerate before running them. Never run them in
  the same `cargo test` invocation as update mode; the regenerate command's
  filter already prevents this.
