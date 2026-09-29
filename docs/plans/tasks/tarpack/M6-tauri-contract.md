# M6 — Tauri commands, events, watcher, and the typed client

Status: awaiting approval
Project: tarpack   Depends on: M4 (landed), M5 (landed); M3's partial-results
follow-up (landed before M4 and M5)

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
- **M3:** `fm_tarpack::manifest::{load, LoadedManifest, LoadError, ParseReport, EntryFailure, Diagnostic, ManifestView, EntryView}`.
  `load(path)` returns `LoadedManifest { path, sha256, report: ParseReport }`,
  and fails only with `LoadError::Io` (unreadable file). A readable file
  always loads, whatever its errors. `LoadedManifest.sha256` hashes the exact
  file bytes. `ParseReport` is
  `{ manifest, entries_withheld, errors, failures: Vec<EntryFailure>, warnings }`
  with `is_valid()` and `error_count() -> u32`:
  - `manifest` holds only the entries that passed; `Manifest::is_complete()`
    equals `is_valid()`;
  - `errors` are the manifest-level errors (outside every `[[file]]` table);
  - `failures` has one `EntryFailure { index, id, source, line, errors }` per
    `[[file]]` table with errors, in manifest order (`EntryFailure` is
    already exported to `lib/generated/`);
  - `entries_withheld` is `true` when a manifest-level error (syntax, not
    UTF-8, `version`, `[defaults]`, an unknown top-level key, `file` not an
    array) means no entry can be trusted, so the manifest has no entries;
  - when `name` is in error the manifest's name is `""`, and when
    `output_name` is in error it is `None`.

  The rules are in `docs/tarpack-manifest.md` ("When the manifest has
  errors"). The format is in
  `docs/tarpack-manifest.md`. `ManifestView::from(&Manifest)` gives
  `{ name, output_name, entries: Vec<EntryView> }` (no default format), and
  `EntryView` is `{ id, source, target_path, mode, mode_text, owner, uid, gid, normalize_eol }`
  in `crates/fm-tarpack/src/manifest/model.rs`, serialised camelCase:
  `mode` is `"0755"`, `modeText` is `"rwxr-xr-x"`, `owner` is `"root:root"`.
  Both are already exported to `lib/generated/`.
- **M3:** `fm_tarpack::format::ArchiveFormat { Tar, TarGz, TarZst, TarXz }`
  with `ALL`, `extension()`, `from_file_name()` (recognises `.tar`,
  `.tar.gz`, `.tgz`, `.tar.zst`, and `.tar.xz`, case-insensitively), and
  `with_extension()`, and `filter_extension()` (`"tar"`, `"gz"`, `"zst"`,
  or `"xz"`: the last suffix without its dot, as a Save-dialog filter needs).
  `Manifest::default_format()` exists too.
- **M4:** `fm_tarpack::archive::{ArchivePlan, PlanError, write_archive, BuildSummary, BuildError, Progress, BuildPhase}`.
  `ArchivePlan::new` fails with `PlanError::ManifestIncomplete` for a
  manifest that is not complete, and with `PlanError::Unassigned(ids)`.
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
  `restore_all` (every remembered source for the manifest, unfiltered), and
  `touch_recent`.

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
- **Nothing is built while the manifest has any error** (decided by the
  human, with partial results). Building only the entries that passed would
  silently leave out files the manifest lists. The session shows the passed
  entries, reports the failed ones, and tells the UI how many errors there
  are, but `canBuild` stays false until the error count is 0.
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
    entries: [{ ...EntryView,        // id, source, targetPath, mode, modeText, owner, uid, gid, normalizeEol
                assigned: string|null, status: "ready"|"missing"|"unassigned" }],
                                     // passed entries only, manifest order
    entriesWithheld: boolean,        // a manifest-level error hides every entry; entries is []
    errors: Diagnostic[],            // manifest-level errors only (outside every [[file]] table)
    failedEntries: EntryFailure[],   // one per [[file]] table with errors, manifest order
    errorCount: number,              // errors.length + every failedEntries[i].errors.length
    warnings: Diagnostic[]
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

**The entry type reuses M3's `EntryView`; do not redefine its fields.** Define
the session entry as a struct with `#[serde(flatten)] #[ts(flatten)] view: EntryView`
plus `assigned: Option<String>` and `status`, so the generated TS is
`EntryView & { assigned, status }` (or ts-rs's equivalent inlined object). Take
`name`, `outputName`, and the `EntryView`s from `ManifestView::from(&manifest)`;
the session manifest adds `path`, `hash`, `errors`, and `warnings` around
them. The session does not carry a separate default format: `format` is the
only format field the UI reads. Add a test that serialises one session entry
and asserts the keys `mode` (`"0755"`), `modeText`, `owner` (`"root:root"`),
`uid`, `gid`, `assigned`, and `status` sit at the same level.

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

**A manifest with errors** (partial results, decided by the human):

- `tarpack_open_manifest`, `tarpack_reload_manifest`, and the first
  `tarpack_session()` restore succeed for any readable file, whatever its
  errors. Only an unreadable file fails, with `ManifestUnreadable`. The
  watcher is armed either way, so the user's fix is noticed.
- The session manifest comes from M3's `ParseReport`: `entries` from
  `ManifestView::from(&report.manifest)` plus `assigned` and `status`;
  `entriesWithheld`, `errors`, `warnings` as in the report;
  `failedEntries` is `report.failures`, the `EntryFailure` type unchanged
  (no redefinition; it is already generated); `errorCount` is
  `report.error_count()`.
- `errorCount > 0` is the contract's single "there were errors" flag. The UI
  uses it to tell the user, and `canBuild` uses it to block
  (`buildBlockedReason: "manifestInvalid"`). `errorCount` is a `u32`, so it
  generates as `number`.
- `name` is the manifest's name, or, when that is empty (possible only when
  `name` itself is in error), the manifest file's stem, converted lossily for
  display. `outputName` is `null` when `output_name` is in error.
  `suggestedOutputName` then falls back to the file stem, as with no
  `output_name`.
- `readyCount` and `totalCount` count the passed entries only.
- **Working while errors exist.** Assign, clear, drop, `setFormat`, and
  `setOutput` all work on the passed entries, so the user can prepare the
  build while fixing the manifest. The ids of failed or withheld entries are
  not in the manifest: `tarpack_assign` and `tarpack_clear` with one fail
  with `UnknownEntry`, and drop matching never considers them.
- **Remembered sources are not pruned while errors exist.** A typo in one
  entry must not erase that entry's remembered file.
  - When the report is valid, open uses `RememberedState::restore` (ids not
    in the manifest are dropped), and reload keeps only the assignments whose
    ids are in the manifest.
  - When it is not valid, open uses `RememberedState::restore_all`, and
    reload keeps every assignment it holds. The next `remember` therefore
    writes back the sources of failed entries too. The session still lists
    only passed entries.
- **Build.** `tarpack_build` with `errorCount > 0` fails with
  `ManifestInvalid` before re-reading the manifest or touching the output.
  If it ever reached M4, `PlanError::ManifestIncomplete` maps to
  `ManifestInvalid` too.

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
| `ManifestInvalid` | build | The manifest has errors (defensive: `canBuild` already blocks this); also `PlanError::ManifestIncomplete` | — |
| `ManifestChangedOnDisk` | build | The file hash differs from the loaded one | — |
| `UnknownEntry` | assign, clear | No passed entry has that id (this includes the ids of failed entries) | the id |
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
- A manifest with errors opens: the session lists its passed entries, its
  failed entries with all their errors, its manifest-level errors, and
  `errorCount`, and `canBuild` is false with `"manifestInvalid"`. With a
  withholding error, `entries` is empty and `entriesWithheld` is true.
- A failed entry's remembered source survives opening, editing, and
  reloading the invalid manifest, and comes back once the entry is fixed.
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
  - `invalid_manifest_session_carries_passed_and_failed_entries`: a fixture
    with a valid `a`, a `b` with `mode = "9"`, and `name = ""` → `entries`
    is `[a]`, `failedEntries` has `b` with its error, `errors` has the name
    error, `errorCount == 2`, `name` is the file stem, `canBuild` is false
    with `"manifestInvalid"`
  - `withheld_manifest_session_has_no_entries`: `version = 2` and a syntax
    error each give `entriesWithheld: true` and no entries, and the open
    succeeds
  - `assign_rejects_failed_entry_id`: `UnknownEntry` with the id
  - `build_refuses_invalid_manifest`: `ManifestInvalid`, nothing written, an
    existing output unchanged
  - `invalid_manifest_keeps_remembered_source_of_failed_entry`: remember
    sources for `a` and `b`; reopen with `b` broken; assign `a` again
    (a mutating command, so state is persisted); simulate a restart; fix `b`
    on disk and reload → `b`'s source is restored
  - `session_manifest_serialises_failure_fields`: the session manifest has
    the keys `entriesWithheld`, `failedEntries` (each with `index`, `id`,
    `source`, `line`, `errors`), and `errorCount`
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
    every `PlanError` variant (`ManifestIncomplete` → `ManifestInvalid`,
    `Unassigned` → `EntriesNotReady`) maps to its kind and `entryId`
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
