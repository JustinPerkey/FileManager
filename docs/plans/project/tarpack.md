# Project plan: foundation and the Tar Packager subcomponent

Status: **awaiting approval.** Decisions marked *(proposed)* are the planner's
recommendation and stand unless the human overrides them. The open questions at
the end need an answer before the milestone that depends on them starts.

UI project plan: [`tarpack-ui.md`](tarpack-ui.md). Task plans:
[`../tasks/tarpack/`](../tasks/tarpack/).

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

Each milestone is specified in full in its task plan. The task plan is the only
plan its `implementer` reads, so any change to a milestone is made there, and
also here when it changes the index below.

| # | Task plan | Goal | Depends on |
| --- | --- | --- | --- |
| M1 | [`M1-foundation.md`](../tasks/tarpack/M1-foundation.md) | Stack, layout, boundary, commands, CI, `CLAUDE.md` | none |
| M2 | [`M2-core-state-store.md`](../tasks/tarpack/M2-core-state-store.md) | `fm-core` app dirs, namespaced state store, atomic write | M1 |
| M3 | [`M3-manifest.md`](../tasks/tarpack/M3-manifest.md) | Manifest model, parsing, validation; `docs/tarpack-manifest.md` | M1 |
| M4 | [`M4-archive-writer.md`](../tasks/tarpack/M4-archive-writer.md) | Atomic, verified `.tar` writer | M2, M3 |
| M5 | [`M5-source-matching.md`](../tasks/tarpack/M5-source-matching.md) | Drop/pick matching, remembered locations | M2, M3 |
| M6 | [`M6-tauri-contract.md`](../tasks/tarpack/M6-tauri-contract.md) | Tauri commands, events, watcher, typed `lib/` client | M4, M5 |
| M7 | [`M7-windows-packaging.md`](../tasks/tarpack/M7-windows-packaging.md) | Windows bundle and end-to-end check | M6, U6 |

M2 and M3 can run in parallel, and so can M4 and M5.

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
