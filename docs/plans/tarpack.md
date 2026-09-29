# Plan: foundation and the Tar Packager subcomponent

Status: **awaiting approval.** Decisions marked *(proposed)* are the planner's
recommendation and stand unless the human overrides them. The open questions at
the end need an answer before the milestone that depends on them starts.

UI plan: [`tarpack-ui.md`](tarpack-ui.md).

## 1. Request

A local desktop application, Rust, run only on a Windows desktop. It will grow
several subcomponents ("tools"). The first tool, the **Tar Packager**
(`tarpack`):

1. Reads a **tar manifest**: a config file, editable outside the program, that
   lists a fixed set of files. For each file it gives the Linux directory the
   file goes to in the archive and its permissions.
2. Lets the user supply the actual Windows file for each entry, by drag-and-drop
   or with a file picker.
3. Remembers the last Windows location used for each entry, so repeat test runs
   need no re-selection.
4. Writes a `.tar` file when the user clicks a button.

## 2. Foundation decisions

`CLAUDE.md` says the stack is not yet chosen, so Milestone 1 is the foundation.

### 2.1 Stack *(proposed)*

**Tauri v2 shell + Rust workspace crates + React/TypeScript/Vite frontend.**

- It matches `WarnerRobinsBurgerWeek`, so the agent chain, the `impeccable` UI
  method, design tokens, and the implementer/ui-implementer split carry over
  unchanged.
- Tauri's native drag-drop event delivers real Windows file-system paths, which
  a plain browser drop does not. Its dialog plugin gives native Open and Save
  dialogs.
- All filesystem and archive logic is Rust. The frontend only renders state and
  calls commands.
- Alternative: an all-Rust UI (egui, Slint, or Leptos inside Tauri). This keeps
  the codebase to one language, but gives weaker design-token and accessibility
  tooling and does not fit the `impeccable` pipeline. See open question Q1.

The app runs only on Windows, but every crate outside the Tauri shell must build
and pass its tests on Linux too. That keeps the core testable in any container,
and CI also runs it on Windows.

### 2.2 Layout, designed for many tools

```
Cargo.toml                    workspace
crates/
  fm-core/                    shared, tool-agnostic: app data dirs, versioned
                              JSON state store (namespaced per tool), atomic
                              file write, common error type. No tauri dep.
  fm-tarpack/                 Tar Packager domain: manifest model, parse and
                              validate, source matching, archive writer.
                              Depends on fm-core only. No tauri dep.
apps/desktop/
  src-tauri/                  Tauri v2 shell (binary `filemanager`)
    src/lib.rs                builder, plugins, invoke_handler
    src/tools/mod.rs          tool registry: each tool's commands and state
    src/tools/tarpack.rs      tarpack commands, events, manifest watcher
  src/                        React frontend
    lib/                      ── implementer-owned seam ──
      generated/              TS types generated from Rust (ts-rs)
      tauri.ts                invoke/event/dialog/drag-drop wrappers
      tarpack.ts              typed tarpack API
    tools/registry.ts         list of tools shown in navigation
    tools/tarpack/            tarpack views and components (ui-implementer)
    app/                      shell, navigation, layout (ui-implementer)
    styles/tokens.css         design tokens, light and dark
examples/tarpack/example.toml sample manifest
docs/tarpack-manifest.md      manifest reference
```

**Adding a tool** is a fixed recipe, written into `CLAUDE.md` by Milestone 1:

1. Add a crate `crates/fm-<tool>` with no tauri dependency.
2. Add a module `src-tauri/src/tools/<tool>.rs` whose commands are all prefixed
   `<tool>_` and registered through `tools/mod.rs`.
3. Add a typed wrapper `src/lib/<tool>.ts`.
4. Add a view folder `src/tools/<tool>/` and one entry in `tools/registry.ts`.
5. Keep persisted state under the tool's own namespace in `fm-core`'s store.

Tools never import each other. Anything two tools share moves into `fm-core`.

### 2.3 The core/UI boundary

- It falls at `apps/desktop/src/lib/`. The `implementer` owns `lib/` and
  everything in Rust. The `ui-implementer` owns everything else under
  `apps/desktop/src/`.
- Types that cross the boundary are defined once in Rust with
  `#[derive(ts_rs::TS)]` and generated into `src/lib/generated/`. A test fails if
  the generated files are stale. This makes the "defined once, mirrored" rule
  mechanical.
- Session state (the loaded manifest, the assignments, the output path) lives in
  Rust as Tauri-managed state. Every tarpack command returns a full
  `TarpackSession` snapshot, and the UI renders it. The UI keeps no second copy
  of the truth.

### 2.4 Commands *(proposed, recorded in `CLAUDE.md` by Milestone 1)*

```sh
npm install                                        # once, at repo root
npm run tauri:dev                                  # run the app
npm run tauri:build                                # Windows installer / exe

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run typecheck && npm run lint && npm run test  # frontend (vitest)
```

CI (GitHub Actions) has two jobs:

- **ubuntu-latest:** fmt, clippy, and tests for the `fm-*` crates, plus the
  frontend typecheck, lint, and tests.
- **windows-latest:** tests for the `fm-*` crates, plus `tauri build`. This is
  the authoritative platform.

## 3. The manifest *(proposed format)*

The manifest is TOML: readable, comment-friendly, and safe to hand-edit. The
user opens any manifest file from anywhere on disk. The app also creates and
suggests a default folder, `%APPDATA%\FileManager\tarpack\manifests\`.

```toml
# Tar Packager manifest
version = 1
name = "Gateway deploy"
output_name = "gateway.tar"      # suggested file name in the Save dialog

[defaults]                       # every field optional; shown values are the built-in defaults
mode = "0644"                    # file permissions, octal
dir_mode = "0755"                # permissions for directory entries the archive creates
uid = 0
gid = 0
uname = "root"
gname = "root"

[[file]]
id = "gateway-bin"               # stable key; last-used locations are stored against it
source = "gateway"               # expected Windows file name; used to match drops
dir = "/opt/gateway/bin"         # Linux directory inside the archive
# name = "gateway"               # optional rename; defaults to `source`
mode = "0755"
# uid, gid, uname, gname         # optional per-file overrides

[[file]]
id = "gateway-conf"
source = "gateway.conf"
dir = "/etc/gateway"
mode = "0640"
gname = "gateway"
gid = 990
```

### Validation

Every error reports its line and column. A manifest with any error cannot be
built. The rules:

- `version` is supported.
- Unknown keys are rejected, so typos surface instead of being ignored.
- `id` values are unique and non-empty.
- `dir` is an absolute POSIX path: it starts with `/`, has no `..` segments, no
  backslashes, and no NUL.
- `name` and `source` have no `/` or `\`.
- The final target paths (`dir` + `name`) are unique. Paths are compared
  case-sensitively, because the target is Linux.
- `mode` is octal and at most `07777`.
- `uid` and `gid` fit in `u32`. `uname` and `gname` are at most 32 bytes.

These produce **warnings**, not errors:

- Two entries share a `source` name. A drop is then ambiguous for them, and the
  user must use the per-row picker.
- A target path does not fit a plain ustar header. The writer then uses GNU
  long-name headers.

### Archive semantics *(proposed)*

- Entries are written in manifest order, each file preceded by any parent
  directory entries not yet written.
- Parent directories get `dir_mode` and the default owner. Because the archive
  records them explicitly, `tar -xpf` recreates them with those permissions.
- Paths inside the archive are **relative** (the leading `/` is stripped), which
  is standard tar practice. Extract with `tar -xpf x.tar -C /` (see Q2).
- The GNU header format is used, so long paths work.
- `mtime` is the source file's modification time (see Q7).
- File bytes are copied verbatim. There is no line-ending conversion (see Q6).

## 4. Milestones

### Milestone 1 — Foundation: workspace, Tauri shell, tool registry, CI

**Goal.** Establish the stack, layout, boundary, commands, and CI, and record
them in `CLAUDE.md`.
**Depends on.** None. The visual shell is UI task 1, which builds on this.
**Components / likely files.**

- Root `Cargo.toml`, `rust-toolchain.toml` (stable, pinned), `rustfmt.toml`
- `crates/fm-core`, `crates/fm-tarpack` (empty libs with one smoke test each)
- `apps/desktop/src-tauri/*`: `tauri.conf.json` (Windows target, minimum window
  800×560, dialog plugin, opener plugin; no capabilities beyond those used)
- `apps/desktop/src/lib/tauri.ts`
- `apps/desktop/src/tools/registry.ts` (empty list), a minimal `App.tsx`
  placeholder
- Root `package.json` scripts, ESLint, `tsconfig`, vitest
- The `ts-rs` export and its staleness test
- `.github/workflows/ci.yml`
- `CLAUDE.md`: stack, layout, commands, boundary directory, add-a-tool recipe
- `.env.example`: stating that no keys exist yet

**Architectural change.** Creates everything described in §2.
**Acceptance criteria.**

- Every §2.4 command runs clean.
- `tauri:build` succeeds on the Windows CI job.
- `CLAUDE.md` names `apps/desktop/src/lib/` as the boundary and contains the
  add-a-tool recipe.
- Neither `fm-core` nor `fm-tarpack` depends on `tauri` (checked by
  `cargo tree -p fm-tarpack -i tauri` returning nothing, run in CI).

**Tests proving completion.**

- `cargo test --workspace`: the crate smoke tests, plus
  `generated_types_are_current`.
- `npm run test`: a placeholder render test.
- Both CI jobs are green.

**Existing tests to refactor.** None.
**Compatibility / regression risk.** None; this is a new repository. The Tauri
shell needs webkit2gtk to build on Linux. The Linux CI job therefore does not
build the shell, and that is stated in `CLAUDE.md`.

### Milestone 2 — `fm-core`: app directories and the state store

**Goal.** Give every tool a namespaced, versioned, crash-safe place to keep
state.
**Depends on.** M1.
**Components / likely files.**

- `crates/fm-core/src/{dirs.rs, store.rs, atomic.rs, error.rs}`

**Architectural change.**

- `AppDirs` resolves `%APPDATA%\FileManager\` via the `directories` crate. It is
  also constructible from an explicit root, so tests never touch real app data.
- `Store::<T>::load(dirs, "tarpack", "state")` and `save` read and write
  `<root>/<tool>/<name>.json`. The JSON carries a `schema_version`.
- `atomic_write(path, bytes)` writes a temp file in the same directory, then
  renames it over the target.

**Acceptance criteria.**

- Saving is atomic.
- A corrupt or newer-version state file is not overwritten silently. It is
  renamed to `*.bad-<timestamp>.json`, a default state is used, and a warning is
  returned for the UI to show.
- Two tools' stores cannot collide.

**Tests proving completion.** `cargo test -p fm-core`:

- `store_round_trips`
- `corrupt_state_is_preserved_and_reset`
- `newer_schema_is_not_overwritten`
- `namespaces_are_isolated`
- `atomic_write_leaves_no_partial_file_on_error`

All of them run in `tempfile::TempDir`.
**Existing tests to refactor.** None.
**Compatibility / regression risk.** The state format is versioned from day
one.

### Milestone 3 — Manifest model, parsing, and validation

**Goal.** Turn a manifest file into a validated `Manifest`, or into a list of
located errors and warnings.
**Depends on.** M1.
**Components / likely files.**

- `crates/fm-tarpack/src/manifest/{model.rs, parse.rs, validate.rs, mode.rs}`
- `examples/tarpack/example.toml`
- `docs/tarpack-manifest.md`

**Architectural change.**

- Raw serde types use `deny_unknown_fields` and `toml` spans.
- A separate `Manifest` type can only be constructed through validation, so the
  archive writer never sees an invalid manifest.
- Includes a `ManifestDiagnostics { errors, warnings }` type, with each entry
  carrying `line`, `col`, and a message.
- `LoadedManifest` records the file path and a SHA-256 of the file bytes (used
  in M6 to detect edits made outside the app).

**Acceptance criteria.**

- Every rule in §3 has a passing and a failing case.
- Error messages name the offending entry by `id`.
- The example manifest validates clean.

**Tests proving completion.** `cargo test -p fm-tarpack manifest`, a table-driven
test per rule, including:

- `rejects_relative_dir`
- `rejects_dotdot`
- `rejects_backslash_in_dir`
- `rejects_duplicate_target`
- `rejects_unknown_key_with_location`
- `mode_parses_octal_strings`
- `warns_on_shared_source_name`
- `example_manifest_is_valid`

**Existing tests to refactor.** None.
**Compatibility / regression risk.** `version = 1` is fixed now. A later format
change adds `version = 2` handling; version 1 files are never reinterpreted.

### Milestone 4 — Archive writer

**Goal.** Given a validated manifest and a resolved Windows source path for
every entry, write a correct `.tar` atomically and verify it.
**Depends on.** M2, M3.
**Components / likely files.**

- `crates/fm-tarpack/src/archive/{plan.rs, write.rs, verify.rs}`, using the
  `tar` crate and `sha2`

**Architectural change.**

- `ArchivePlan::new(&Manifest, &Assignments)` is a pure function. It yields the
  ordered entry list, directories included, and fails if any entry lacks a
  source.
- `write_archive(plan, out_path, overwrite, progress: impl FnMut(Progress))`:
  - streams each source into a temp file beside `out_path`;
  - re-reads the result and checks the entry names, modes, owners, and sizes
    against the plan;
  - only then renames it into place;
  - returns a `BuildSummary` with the path, entry count, byte size, and SHA-256.
- If `out_path` exists and `overwrite` is false, it returns `OutputExists` and
  writes nothing.

**Acceptance criteria.**

- The archive round-trips with exact paths, `mode & 0o7777`, uid, gid, uname,
  gname, and mtime.
- Parent directories appear once, before their children, with `dir_mode`.
- A missing or unreadable source, or a mid-write failure, leaves no output file
  and no temp file, and the error names the entry.
- Source bytes are identical in the archive.

**Tests proving completion.** `cargo test -p fm-tarpack archive`:

- `round_trip_preserves_headers`
- `parent_dirs_emitted_once_in_order`
- `long_paths_use_gnu_headers`
- `missing_source_writes_nothing`
- `existing_output_requires_overwrite`
- `summary_hash_matches_file`

`extracts_with_system_tar_preserving_modes` is `#[cfg(unix)]`. It runs
`tar -xpf` into a temp dir and checks `stat` modes. Everything runs in temp
dirs.
**Existing tests to refactor.** None.
**Compatibility / regression risk.** Windows files carry no Unix mode, so modes
always come from the manifest and never from the source file. Tests assert
this.

### Milestone 5 — Source matching and remembered locations

**Goal.** Assign Windows files to manifest entries from drops or picks, and
restore last-used locations per manifest.
**Depends on.** M2, M3.
**Components / likely files.**

- `crates/fm-tarpack/src/sources/{assign.rs, matching.rs, remembered.rs}`

**Architectural change.**

- `Assignments` maps `id` to a Windows `PathBuf`. Each assignment has a status:
  `Ready`, `Missing` (the path no longer exists), or `Unassigned`.
- `match_dropped(&Manifest, &[PathBuf]) -> DropOutcome { matched, unmatched, ambiguous }`:
  - Files match on file name against `source`. Windows file names are
    case-insensitive, so the comparison is too.
  - A dropped folder is searched recursively for the names of still-unassigned
    entries. The search is depth-capped at 8 and skips symlinks and junctions.
  - More than one candidate for an entry is reported as ambiguous and left
    unassigned. The program never guesses.
- `assign(id, path)` accepts any file regardless of its name. The user decides.
- `RememberedLocations`, persisted through the M2 store under `tarpack`:
  - maps the canonical manifest path to an `id → path` map;
  - also records the last output directory and a most-recently-used list of up
    to 10 manifests.
- Remembered paths are restored on load, with their status checked. Ids that no
  longer exist in the manifest are dropped from state on the next save.

**Acceptance criteria.**

- Each `DropOutcome` bucket is populated correctly.
- Assignments survive an app restart (simulated by a new store over the same
  temp root).
- Paths are kept as `PathBuf`/`OsString`, never lossily stringified except for
  display.

**Tests proving completion.** `cargo test -p fm-tarpack sources`:

- `drop_matches_case_insensitively`
- `folder_drop_finds_nested_files`
- `ambiguous_drop_is_not_assigned`
- `unmatched_files_reported`
- `remembered_locations_restore_with_missing_status`
- `removed_ids_are_pruned`

**Existing tests to refactor.** None.
**Compatibility / regression risk.** Manifests are keyed by canonical path, so
renaming a manifest file forgets its remembered locations. This is acceptable
and documented.

### Milestone 6 — Tauri commands, events, watcher, and the typed client

**Goal.** Expose tarpack to the UI through a typed contract.
**Depends on.** M4, M5. **UI tasks 2–5 depend on this milestone** and must not
start before it lands.
**Components / likely files.**

- `apps/desktop/src-tauri/src/tools/{mod.rs, tarpack.rs}`
- `apps/desktop/src/lib/{tarpack.ts, tauri.ts}`
- `src/lib/generated/*`

**Architectural change.**

Commands, each returning `TarpackSession` or a typed error:

- `tarpack_open_manifest(path)`
- `tarpack_reload_manifest()`
- `tarpack_session()`: the current state, including state restored on startup
- `tarpack_assign_dropped(paths)`: also returns the `DropOutcome`
- `tarpack_assign(id, path)`
- `tarpack_clear(id)`
- `tarpack_set_output(path)`
- `tarpack_build(overwrite)`: returns `BuildSummary`. Before building it
  re-reads the manifest from disk. If the hash differs from the loaded one, it
  fails with `ManifestChangedOnDisk` rather than build something the user did
  not see.
- `tarpack_recent_manifests()`
- `tarpack_open_in_editor()` and `tarpack_reveal_output()`, via the opener plugin
- `tarpack_create_manifest_from_example(path)`

Events:

- `tarpack://manifest-changed`: sent by a `notify` watcher on the loaded file,
  debounced by 300 ms.
- `tarpack://build-progress`: `{ entry, bytesDone, bytesTotal }`

The build runs on a blocking thread, so the UI stays responsive.

In `lib/`:

- `lib/tarpack.ts` wraps every command and event.
- `lib/tauri.ts` wraps the dialog plugin and `getCurrentWebview().onDragDropEvent`.

**Acceptance criteria.**

- Every command and event is typed end to end from generated types. No
  hand-written duplicate types.
- The shell's `tauri.conf.json` capabilities grant only the dialog, opener, and
  event permissions used.
- An external edit to the manifest produces exactly one `manifest-changed`
  event.
- Building after an unseen edit fails safely.

**Tests proving completion.**

- `cargo test -p filemanager tools::tarpack`: the command logic tested through
  plain functions over an injected `AppDirs`. Tauri state is thin glue.
- `build_refuses_when_manifest_changed`
- `npm run test`: `lib/tarpack.test.ts`, with `@tauri-apps/api` mocked.
- The generated-types staleness test.

**Existing tests to refactor.** The M1 placeholder test.
**Compatibility / regression risk.** Command names are prefixed per tool, so
future tools cannot collide.

### Milestone 7 — Windows packaging and end-to-end check

**Goal.** Produce a Windows build a user can install and run, and prove the
whole flow on Windows.
**Depends on.** M6 and UI task 6.
**Components / likely files.**

- `tauri.conf.json` bundle settings
- The CI artifact upload
- `docs/tarpack-e2e.md`: a manual checklist

**Acceptance criteria.**

- CI uploads the Windows bundle (the format depends on Q8).
- The checklist passes on a real Windows machine:
  1. Open the example manifest.
  2. Drop a folder, then pick one file.
  3. Build.
  4. Restart the app; paths are restored.
  5. Edit the manifest externally; the banner appears.
  6. Extract on Linux with `tar -xpf ... -C <tmp>`; the modes and owners match.

**Tests proving completion.** The Windows CI job is green, and the checklist
result is recorded in the PR.
**Compatibility / regression risk.** Code signing is out of scope, so the
unsigned build shows a SmartScreen prompt.

## 5. Handoff to ui-designer

The UI is written in [`tarpack-ui.md`](tarpack-ui.md). Its constraints are:

- The UI renders `TarpackSession` and never keeps its own copy.
- It calls only `lib/` wrappers.
- Build is disabled with a stated reason unless the manifest is valid, every
  entry is `Ready`, and an output path is set.
- Overwriting an existing output needs an explicit confirmation.
- A drop shows its matched, unmatched, and ambiguous results.

The ordering is:

- UI task 1 needs only M1.
- UI tasks 2–5 need M6.
- M7 needs UI task 6.

## 6. Open questions for the human

1. **Q1 — UI technology.** Tauri + React/TypeScript (recommended; matches
   BurgerWeek), or an all-Rust UI (egui/Slint)?
2. **Q2 — Archive paths.** Relative `opt/...`, extracted with `-C /`
   (recommended), or absolute `/opt/...`?
3. **Q3 — Compression.** Plain `.tar` only, or also offer `.tar.gz`?
4. **Q4 — Owners.** Is numeric uid/gid plus names per file (defaulting to
   root:root) what the target systems need?
5. **Q5 — Manifest location.** Open from anywhere, with a default folder under
   `%APPDATA%` (recommended), or one fixed location?
6. **Q6 — Line endings.** Files edited on Windows may have CRLF, which breaks
   shell scripts on Linux. Should there be an opt-in per-file
   `normalize_eol = true`? The proposed default is no conversion.
7. **Q7 — mtime.** Use the source file's time (proposed), or the build time?
8. **Q8 — Distribution.** Installer (MSI/NSIS), or a portable `.exe`?
