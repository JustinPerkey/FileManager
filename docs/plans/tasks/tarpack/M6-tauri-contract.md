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
- **M4:** `fm_tarpack::archive::{ArchivePlan, write_archive, BuildSummary, BuildError, Progress}`
- **M5:** `fm_tarpack::sources::{Assignments, EntryStatus, match_dropped, apply, DropOutcome, RememberedState}`

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
                assigned: string|null, status: "ready"|"missing"|"unassigned" }],
    errors: Diagnostic[], warnings: Diagnostic[]
  },
  outputPath: string | null,
  readyCount, totalCount,
  canBuild: boolean,
  buildBlockedReason: null | "noManifest" | "manifestInvalid" | "entriesNotReady" | "noOutput",
  stateWarning: string | null        // from fm-core StoreWarning
}
```

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
| `tarpack_set_output(path)` | `TarpackSession` | |
| `tarpack_build(overwrite)` | `BuildSummary` | Runs on `spawn_blocking`. Errors: `OutputExists`, `ManifestChangedOnDisk`, `SourceMissing`, … |
| `tarpack_recent_manifests()` | `string[]` | |
| `tarpack_open_in_editor()` | `()` | Opener plugin, on the manifest path. |
| `tarpack_reveal_output()` | `()` | Opener plugin; reveal the output in Explorer. |
| `tarpack_create_manifest_from_example(path)` | `TarpackSession` | Writes the bundled `examples/tarpack/example.toml` (`include_str!`). Errors if the path exists. |

Every mutating command persists `RememberedState` afterwards.

**Events.**

- `tarpack://manifest-changed` carries `{ path }`. It is sent by a watcher on
  the loaded manifest file, debounced at 300 ms, and re-armed on open. A new
  open or reload replaces the watcher.
- `tarpack://build-progress` carries `{ entryId, bytesDone, bytesTotal }`.

**`lib/tauri.ts`** adds typed wrappers:

- `openFileDialog({ filters?, defaultPath? })` and
  `saveFileDialog({ defaultPath })`, over the dialog plugin
- `onDragDrop(handler)`, over `getCurrentWebview().onDragDropEvent`, which
  yields `{ type: "enter"|"over"|"leave"|"drop", paths }`
- `listen` and `invoke`, typed over the generated types

**`lib/tarpack.ts`** exports one async function per command, `onManifestChanged`,
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
  - `watcher_emits_once_per_edit`: allowed to be `#[ignore]` on CI if
    file-watch timing is flaky there; must run locally
- `npm run test`: `lib/tarpack.test.ts` and `lib/tauri.test.ts`, with
  `@tauri-apps/api` mocked, checking that each wrapper calls the right command
  with the right arguments.
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
