# Project plan: foundation and the Tar Packager subcomponent

Status: **awaiting approval to start implementation.** The human answered every
open question in §6 on 2026-09-29. The answers are recorded in §6.1 and folded
into §2–§4 and into every backend task plan. Decisions still marked
*(proposed)* are the planner's recommendations that follow from those answers
(§6.2). They stand unless the human overrides them.

On 2026-09-29 the human overruled decision 34 ("building is blocked while any
error exists"): *"A single error does not block builds but is included as an
error in the final report."* §3.2.1 states the replacement rules, and
decisions 37–41 record them with their reasons. The M3 partial-results
follow-up, M4, and M6 are updated to match. The UI consequences are handed to
the ui-designer (§5).

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
4. Writes a `.tar` file when the user clicks a button. The human later added
   compressed variants (§3.4).

**Target system** (from the Q3 answer): armv7, low power, GNU tar with zstd
available.

## 2. Foundation decisions

`CLAUDE.md` says the stack is not yet chosen, so Milestone 1 is the foundation.

### 2.1 Stack *(decided, Q1)*

**Tauri v2 shell + Rust workspace crates + React/TypeScript/Vite frontend.**

- It suits the agent chain as written: the `impeccable` UI
  method, design tokens, and the implementer/ui-implementer split carry over
  unchanged.
- Tauri's native drag-drop event delivers real Windows file-system paths, which
  a plain browser drop does not. Its dialog plugin gives native Open and Save
  dialogs.
- All filesystem and archive logic is Rust. The frontend only renders state and
  calls commands.
- Rejected alternative: an all-Rust UI (egui, Slint, or Leptos inside Tauri).
  It keeps the codebase to one language, but gives weaker design-token and
  accessibility tooling and does not fit the `impeccable` pipeline.

The app runs only on Windows, but every crate outside the Tauri shell must build
and pass its tests on Linux too. That keeps the core testable in any container,
and CI also runs it on Windows.

**Native build dependencies (consequence of Q3).** The `zstd` and xz crates
compile bundled C sources. Windows builds therefore need the MSVC toolchain
(already required by Tauri), and Linux CI needs a C compiler (`cc`/`gcc`,
preinstalled on `ubuntu-latest`). `CLAUDE.md` records this as a prerequisite in
M1. M7 links the C runtime statically, so the portable exe has no DLL
dependencies beyond the system and WebView2.

### 2.2 Layout, designed for many tools

```
Cargo.toml                    workspace; [workspace.dependencies] pins ts-rs
.cargo/config.toml            [env] TS_RS_EXPORT_DIR -> target/ (M1); static CRT (M7)
.gitattributes                lib/generated/** eol=lf (Windows CI checkout)
crates/
  fm-core/                    shared, tool-agnostic: app data dirs, versioned
                              JSON state store (namespaced per tool), atomic
                              file write, common error type. No tauri dep.
  fm-tarpack/                 Tar Packager domain: manifest model, parse and
                              validate, archive formats, source matching,
                              archive writer. Depends on fm-core only.
                              No tauri dep.
apps/desktop/
  src-tauri/                  Tauri v2 shell (binary `filemanager`)
    src/lib.rs                builder, plugins, then tools::register(); never
                              invoke_handler or setup
    src/tools/mod.rs          tool registry: the only .invoke_handler (one
                              generate_handler! for all tools), the only
                              .setup, and every tool's .manage(...)
    src/tools/tarpack.rs      tarpack commands, state, manifest watcher
    src/generated_types.rs    #[cfg(test)] ts-rs export: currency check and
                              opt-in update mode (owns lib/generated/)
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
docs/tarpack-manifest.md      manifest and archive reference, incl. extraction
```

**Adding a tool** is a fixed recipe, written into `CLAUDE.md` by Milestone 1:

1. Add a crate `crates/fm-<tool>` with no tauri dependency. If it has
   boundary types, add `ts-rs = { workspace = true }` and derive `ts_rs::TS`.
2. Add a module `src-tauri/src/tools/<tool>.rs`. It contributes only
   `#[tauri::command]` functions prefixed `<tool>_`, a managed-state
   constructor if needed, and optionally a `setup(app)` function. It never
   touches the `Builder`.
3. In `tools/mod.rs::register`, add the tool's state with `.manage(...)`, add
   its commands to the **single** `tauri::generate_handler![...]`, and call its
   `setup` from the single `.setup(...)`.
4. Append its boundary types to `EXPORTERS` in
   `src-tauri/src/generated_types.rs` and run the regenerate command (§2.4).
5. Add a typed wrapper `src/lib/<tool>.ts`, a view folder `src/tools/<tool>/`,
   and one entry in `tools/registry.ts`.
6. Keep persisted state under the tool's own namespace in `fm-core`'s store.

**Why one handler** (M1 review, P2-3): Tauri's `Builder::invoke_handler` and
`Builder::setup` each replace any earlier call. A registry shaped as "each tool
calls `invoke_handler` on the builder" silently drops every tool's commands
but the last one. Only `tools::register` calls them, once each, and a
shell-crate test (`builder_hooks_only_in_tool_registry`) enforces it.

Tools never import each other. Anything two tools share moves into `fm-core`.

The hook test skips comment lines (lines whose trimmed start is `//`), so doc
comments may name `invoke_handler` and `setup` freely (re-review A).

### 2.3 The core/UI boundary

- It falls at `apps/desktop/src/lib/`. The `implementer` owns `lib/` and
  everything in Rust. The `ui-implementer` owns everything else under
  `apps/desktop/src/`.
- Types that cross the boundary are defined once in Rust with
  `#[derive(ts_rs::TS)]` and generated into `src/lib/generated/`. A test fails if
  the generated files are stale. This makes the "defined once, mirrored" rule
  mechanical.
- **Who generates** (M1 review, P1-1 and P1-2):
  - The export is a `#[cfg(test)]` module in the shell crate,
    `apps/desktop/src-tauri/src/generated_types.rs`. The shell is the only
    crate that depends on every `fm-*` crate, and M6's own boundary types live
    in its private `tools` module, which only an in-crate unit test can name.
    The first implementation put the test in `fm-tarpack`, which cannot see
    either.
  - One `EXPORTERS` list names every root type. `generated_types_are_current`
    compares a fresh temp-dir export with the committed folder.
  - With `UPDATE_GENERATED=1`, the same test rewrites the folder: it clears
    everything except `.gitkeep`, then copies the fresh export in. It refuses
    to run when `CI` is set.
  - `#[ts(export)]` is not used. `.cargo/config.toml` points
    `TS_RS_EXPORT_DIR` into `target/` as a backstop, so a stray one cannot
    write `./bindings` into a crate.
  - `.gitattributes` forces LF in `lib/generated/`, because the Windows runner
    checks out CRLF by default and the byte comparison would fail.
- **One `ts-rs` path** (P3-10): `ts-rs` is declared once in
  `[workspace.dependencies]`, and each crate with boundary types depends on it
  directly (`{ workspace = true }`). The derive expands to `::ts_rs` paths, so
  the first implementation's `pub use ts_rs` re-export from `fm-core` is
  dropped.
- Session state (the loaded manifest, the assignments, the output path, the
  chosen archive format) lives in Rust as Tauri-managed state. Every tarpack
  command returns a full `TarpackSession` snapshot, and the UI renders it. The
  UI keeps no second copy of the truth, and does no extension or format logic
  of its own: the backend supplies the extension list, the suggested file name,
  and the extraction command.

### 2.4 Commands *(recorded in `CLAUDE.md` by Milestone 1)*

```sh
npm install                                        # once, at repo root
npm run tauri:dev                                  # run the app
npm run tauri:build                                # portable Windows exe (tauri build --no-bundle)

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run typecheck && npm run lint && npm run test  # frontend (vitest; lint includes jsx-a11y)
UPDATE_GENERATED=1 cargo test -p filemanager --lib generated_types_are_current   # regenerate lib/generated/
```

`npm run lint` includes `eslint-plugin-jsx-a11y` (recommended), from M1
(P3-9). Accessibility is a requirement, so the lint is in place before the
first UI task, not added by it.

CI (GitHub Actions) has two jobs. **Both run `cargo test --workspace`**, so
the shell crate's tests run on both platforms (P1-2). Those include the
generated-types checks and M6's `tools::tarpack` tests.

- **ubuntu-latest:**
  - installs `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev`, and
    `libjavascriptcoregtk-4.1-dev`, then builds the frontend;
  - runs fmt, then clippy and tests for the whole workspace;
  - runs the frontend typecheck, lint, and tests;
  - runs the tauri-dependency check on the `fm-*` crates.

  It needs a C compiler, which is preinstalled.
- **windows-latest:**
  - builds the frontend;
  - runs `cargo test --workspace`, then `tauri build --no-bundle`, on the
    MSVC toolchain.

  This is the authoritative platform. M7 uploads the exe as an artifact.

Rejected alternative: keep Linux on `fm-*` only and run the workspace tests
only on Windows. That leaves the fast job blind to the shell crate, and the
Linux container used by agents already has the webkit packages. Running both
costs CI minutes and nothing else.

**Tauri-dependency check** (P2-4, re-review B): for each `fm-*` crate:

1. `cargo pkgid -p <crate>` must succeed. With `-i`, `cargo tree` ignores a
   `-p` that matches nothing: it exits 0 and prints the whole inverse tree. So
   without this step a misspelled name would pass, or fail for the wrong
   reason.
2. `cargo tree -p <crate> -i tauri -e normal,build,dev` must then fail with
   cargo's "did not match any packages" message.

The first implementation passed on any non-zero exit. Success (tauri found)
or any other failure now fails the step.

### 2.5 Distribution *(decided, Q8)*

A **portable `.exe`**, no installer. `npm run tauri:build` runs
`tauri build --no-bundle`. The frontend is embedded in the binary.
`tauri.conf.json` has `bundle.active: false` and **no `bundle.targets`**. The
first implementation's `"targets": ["nsis"]` named an installer and is removed
(M1 review, P3-6). Consequences M7 handles:

- The C runtime is linked statically (`+crt-static` for
  `x86_64-pc-windows-msvc` in `.cargo/config.toml`), so the exe runs without
  the VC++ redistributable or the `api-ms-win-crt-*` DLLs. The `cc`-built
  zstd and liblzma objects follow the same CRT setting (`/MT`). This needs
  `build.windows.staticVCRuntime: false` in `tauri.conf.json` (decision 52):
  tauri-build 2.7.0 defaults it to `true` and then links a hybrid CRT
  (static vcruntime, **dynamic** UCRT via `/NODEFAULTLIB:libucrt.lib` and
  `/DEFAULTLIB:ucrt.lib`), which undoes `+crt-static` for the UCRT. A
  `RUSTFLAGS` environment variable (replaces the config's rustflags) or the
  deprecated `STATIC_VCRUNTIME` (overrides the config) would also undo it,
  so neither is set. The Windows CI job proves the result with
  `dumpbin /dependents` (decision 53).
- The exe relies on the Evergreen WebView2 runtime already installed on
  Windows 10/11. It cannot bootstrap it. The README states the requirement.
- State stays in `%APPDATA%\FileManager\` (Q5), not beside the exe. "Portable"
  means no installer, not a self-contained data folder.
- The WebView2 rendering of `MiddlePath`'s middle truncation is checked in
  M7's Windows end-to-end check (step 10 of `docs/tarpack-e2e.md`), run by
  the user before M7 is signed off. It moved there from U6 after U6's review
  (`tarpack-ui.md` §13), because the implementers' container is Linux. A
  failure becomes a follow-up UI task; it neither reopens U6 nor is fixed in
  M7.

### 2.6 Webview security *(decided by the planner, M1 review P3-7)*

The first implementation shipped `"csp": null`. M1 replaces it with:

```
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';
img-src 'self' data:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost;
object-src 'none'; base-uri 'self'; form-action 'none'; frame-ancestors 'none'
```

- The app loads nothing remote. `connect-src ipc: http://ipc.localhost` is
  what Tauri v2 IPC needs on Windows.
- Inline styles are allowed because React `style` attributes are a normal UI
  tool (for example a progress bar width). Inline scripts are not allowed.
  `dangerousDisableAssetCspModification: ["style-src"]` stops Tauri appending
  hashes to `style-src`, which would otherwise make browsers ignore
  `'unsafe-inline'`. Tauri still hashes `script-src`.
- `devCsp` adds the Vite dev server (`ws://` and `http://localhost:1420`) to
  `connect-src`. If React-refresh needs it, `devCsp` also gets
  `script-src 'unsafe-inline'`, and the production policy stays unchanged.
- `tauri:dev` cannot run in the Linux agent container, so `devCsp` is
  unverified at implementation time. A one-time `npm run tauri:dev` on
  Windows, done by the human or orchestrator, is part of M1 landing
  (re-review C). If it shows a CSP violation, only `devCsp` is loosened.
- Changing the CSP is a reviewed decision, recorded in `CLAUDE.md`
  Invariants.

## 3. The manifest and the archive

### 3.1 Manifest format *(decided)*

The manifest is TOML: readable, comment-friendly, and safe to hand-edit. The
user opens any manifest file from anywhere on disk. The app also creates and
suggests a default folder, `%APPDATA%\FileManager\tarpack\manifests\` (Q5).

```toml
# Tar Packager manifest
version = 1
name = "Gateway deploy"
output_name = "gateway.tar.zst"  # suggested Save name; a known archive suffix also picks the default format

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

[[file]]
id = "gateway-start"
source = "start.sh"
dir = "/opt/gateway/bin"
mode = "0755"
normalize_eol = true             # opt-in: CRLF -> LF when writing (Q6)
```

Owners (Q4): numeric uid/gid plus user/group names on every entry, defaulting
to `0`/`0` `root:root`.

`normalize_eol` (Q6) is per file only, boolean, default `false`. It is not
accepted in `[defaults]`: conversion must be a deliberate per-file choice.

### 3.2 Validation

Every error reports its line and column. Every error inside a `[[file]]`
table, including unknown keys, wrong types, and missing fields, names that
entry's `id`. A table without a non-empty string `id` (no `id`, a non-string
one, or `id = ""`) is named by position, `[[file]] #<n>: `, for every error
from it, including `id must not be empty` (M3 re-review). A manifest with
errors still builds from the entries that passed, and the build's report
names everything it left out; §3.2.1 says what is shown and what is built.
The rules:

- `version` is supported.
- The top-level `name` is non-empty. (That is its only rule; the separator
  rules below are for the per-file `name`.)
- Unknown keys are rejected, so typos surface instead of being ignored.
- `id` values are unique and non-empty. A duplicate is reported on every
  table sharing the id.
- `dir` is an absolute POSIX path: it starts with `/`, has no `..` or `.`
  segments, no empty segment except a trailing `/`, no backslashes, and no
  NUL. `.` is rejected because `/opt/./x` and `/opt/x` are the same directory
  and would slip past the duplicate-target check (M3 review B2).
- Per-file `name` and `source` are non-empty, have no `/`, `\`, or NUL, and
  are not `.` or `..`.
- `output_name`, when present, is non-empty and has no `/`, `\`, or NUL.
- The final target paths (`dir` + `name`) are unique. Paths are compared
  case-sensitively, because the target is Linux. The error is reported on
  both entries, each pointing at its `name` when set, else at its `dir`.
- No target path is also a directory of another entry (equal to another
  entry's `dir` or an ancestor of it), since the archive cannot hold
  `/opt/gateway` as both a file and a directory (M3 review N5). Reported on
  both entries.
- `mode` and `dir_mode` are octal strings of 1 to 4 digits.
- `uid` and `gid` fit in `u32`. `uname` and `gname` are 1 to 32 bytes.
- `normalize_eol` is a boolean.

**Reporting.** A TOML syntax error stops at the first. Everything else
(unknown keys, missing fields, wrong types, and the rules above) is collected
and reported together. The only limit is that one `[defaults]` or `[[file]]`
table reports at most one wrong-type or missing-field error, and its value
rules wait until it deserializes (M3 review B3). `docs/tarpack-manifest.md`
states this.

These produce **warnings**, not errors:

- Two entries share a `source` name. A drop is then ambiguous for them, and the
  user must use the per-row picker.
- A stored path (the absolute target path, leading `/` included) is 100 bytes
  or longer. The writer then emits a GNU long-name record for it.
- The manifest has no `[[file]]` entries.

### 3.2.1 Partial results *(decided by the human, 2026-09-29)*

The human's answer to the gap "an invalid manifest shows whatever entries
were readable, but `parse` returns none": *show the ones that passed, collect
the ones that failed in a report, and notify the user that there were
errors.* The planner's rules for it:

- **Passed** means no error is attributed to the entry's `[[file]]` table.
  Warnings never fail an entry.
- **Failed** entries become one `EntryFailure { index, id, source, line, errors }`
  each, in manifest order, with every error of that table.
- **Cross-entry errors fail every entry involved**: all tables sharing a
  duplicate id (assignments and remembered sources are keyed by id, so
  showing one would attach the other's file), both entries of a duplicate
  target, and both entries of a file/directory collision. Each gets its own
  diagnostic. Target comparisons see only tables that resolved, so fixing a
  failed entry can reveal a new collision.
- **Manifest-level errors that withhold every entry**: TOML syntax, a file
  that is not UTF-8, `version` (missing, wrong type, not 1), any `[defaults]`
  error, an unknown top-level key (possibly a misspelt `[defaults]`), and
  `file` not being an array. Each means no entry can be read as its author
  meant it. Entry failures from the same pass are still reported.
- **Manifest-level errors that do not**: `name` and `output_name`. The name
  falls back to the file stem in the session, and `output_name` to none.
- **Errors do not block building** *(decided by the human, 2026-09-29,
  overruling the earlier "blocked while any error exists")*. The human's
  words: *"A single error does not block builds but is included as an error
  in the final report."* The build writes the passed entries. Every failed
  entry, with all its errors, and every manifest-level error is carried into
  the build's final report as an error; warnings are carried too and never
  block. That is how the "never silent" invariant is met without blocking:
  nothing the manifest lists is left out of an archive unless the result of
  that same build names it, and the build bar says beforehand how many
  entries will be left out.
- **Nothing to build.** With no passed entries (entries withheld, every entry
  failed, or the manifest lists none) there is nothing to put in an archive,
  so the build is unavailable with a stated reason: M6's
  `buildBlockedReason: "noEntries"` and error kind `NoEntries`, M4's
  `PlanError::NoEntries`. This is the only way errors affect whether a build
  can run. It also covers a valid manifest with no `[[file]]` entries, which
  before could build an empty archive.
- **No extra confirmation.** Building with errors opens no dialog. The
  pre-build state (the build bar) says how many entries will be left out and
  that the errors will be listed in the result. The overwrite confirmation is
  unchanged and is still the only build dialog.
- **The report is part of the build's result.** M4's `ArchivePlan::new` takes
  the whole `ParseReport`, not a bare `Manifest`, and copies the failures,
  the manifest-level errors, and the warnings into the plan; `write_archive`
  returns them in `BuildSummary` (`builtIds`, `leftOut: EntryFailure[]`,
  `manifestErrors`, `warnings`, `errorCount`). No code path can plan an
  archive from the passed entries without carrying what was left out.
- **Where the report goes.** It is returned to the UI in `BuildSummary` and
  shown in the build result. Nothing is written into the archive, which
  holds exactly the passed entries and their directories, and no sidecar or
  log file is written: the plans define none, and a file beside the output
  would be one the user did not ask for and the target does not expect.
- `Manifest::is_complete()` stays, as a description (the manifest holds every
  entry its file lists), not as a build gate.
- **Working while errors exist.** The user may assign files to passed
  entries and pick the format and output. The remembered sources of failed
  entries are not pruned while errors exist (M5's `restore_all`, used by
  M6), so a typo in one entry does not erase its remembered file.
- **Notification.** The session carries `errorCount`
  (manifest-level errors plus every failed entry's errors). The UI tells the
  user whenever it is above 0, and a build's result repeats the count and
  the errors. How is the ui-designer's decision.

API: `parse(text) -> ParseReport { manifest, entries_withheld, errors, failures, warnings }`
and `load(path) -> Result<LoadedManifest { path, sha256, report }, LoadError::Io>`.
Only `EntryFailure` is a new boundary type. `ManifestView`, `EntryView`, and
`Diagnostic` are unchanged.

### 3.3 Archive semantics *(decided)*

- Entries are written in manifest order, each file preceded by any parent
  directory entries not yet written. The root `/` itself is never emitted.
- Parent directories get `dir_mode` and the default owner.
- **Paths stored in the archive are absolute** (Q2): `/opt/gateway/bin/gateway`,
  and directory entries `/opt/`, `/opt/gateway/`, and so on. The human chose
  this knowing that GNU and BusyBox tar strip the leading `/` unless `-P` is
  given. The target extracts with GNU tar using `-P`.
- **Header writing.** The GNU header format is used. The Rust `tar` crate's
  `Header::set_path` rejects absolute paths, so M4 writes the name bytes into
  the header's `name` field directly and appends with `Builder::append` (which
  does not re-validate the path). Names of 100 bytes or more get a preceding GNU
  long-name record (`././@LongLink`, typeflag `L`, the name plus a NUL as its
  data). The ustar `prefix`/`name` split is not used, because GNU headers
  reuse the prefix bytes for other fields. Mixing the two formats would produce
  headers that some readers misparse. Tests assert that every stored name
  begins with `/`.
- **mtime** is the source file's modification time (Q7), in whole seconds.
  Directory entries take the newest source mtime in the archive.
- **Bytes** are copied verbatim, unless the entry has `normalize_eol = true`
  (Q6). Then every CRLF pair becomes LF. Lone CR bytes are left alone. The
  header size is the converted size, which the writer learns in a counting pass
  before it writes the header. If the second pass yields a different size, the
  source changed mid-build, and the build fails naming the entry.

### 3.4 Output formats *(decided, Q3; parameters decided by the planner)*

The user picks one of four formats at build time:

| Format | Extension | Crate | Setting | Decompression memory on target |
| --- | --- | --- | --- | --- |
| Uncompressed | `.tar` | — | — | none |
| gzip | `.tar.gz` | `flate2` (pure-Rust `miniz_oxide` backend) | level 6 | ~32 KiB window |
| zstd | `.tar.zst` | `zstd` (bundled libzstd) | level 19, `window_log` capped at 23, content checksum on, no long mode | ~8 MiB window + small buffers |
| xz | `.tar.xz` | `liblzma` (maintained fork of `xz2`, same API; bundled source, static) | preset 6, CRC64 check, single-threaded | ~9 MiB |

Reasons:

- **Decompression cost is paid on a low-power armv7.** For both zstd and xz,
  decompression memory is set by the window or dictionary size, not by the
  compression level. Compression cost is paid on the Windows desktop, where it
  is cheap.
- **zstd:** level 19 gives near-maximum ratio. Its default window for large
  inputs is 8 MiB. Setting `window_log = 23` explicitly makes the bound hold
  whatever the libzstd version's defaults, and it stays within the default
  decoder limit (`--memory=128MB`) with no flags on the target. Long-distance
  mode (`--long`) is off, because it raises the window to 128 MiB and needs a
  matching flag to decompress. Levels 20–22 ("ultra") raise the window and are
  rejected. zstd is the recommended format for this target: its decompression
  is several times faster than xz on ARM.
- **xz:** preset 6 is xz's default, with an 8 MiB dictionary and about 9 MiB to
  decompress. Presets 7–9 need 17–65 MiB to decompress and are rejected.
  `-e`/extreme is not used; it barely helps and costs compression time.
  Multithreaded encoding is not used, because it splits the stream into blocks
  and changes memory characteristics on the decoder for no gain here.
- **gzip:** level 6 is the conventional default. The window is fixed at 32 KiB,
  so the target cost is negligible. The pure-Rust backend needs no C.
- **Integrity:** zstd's content checksum and xz's CRC64 let the target's
  `tar` detect corruption. gzip carries CRC32 by definition.

The format parameters live as named constants in
`crates/fm-tarpack/src/format.rs`, with these reasons as doc comments.

**Format selection and file name.**

- `ArchiveFormat` is a Rust enum `{ Tar, TarGz, TarZst, TarXz }` exported to
  TS. It lives in `fm_tarpack::format`, created by M3, so that M4 (encoders) and
  M5 (remembered choice) can run in parallel.
- **Default format when a manifest opens:** the last format remembered for that
  manifest. Otherwise the format implied by the suffix of `output_name` if it is
  a known archive suffix (`.tar`, `.tar.gz`, `.tgz`, `.tar.zst`, `.tar.xz`,
  compared case-insensitively). Otherwise `.tar`.
- **Remembered per manifest** *(decided, recommended by the human)*: the last
  chosen format is stored with the manifest's other remembered data in the
  tool's state store, and restored on open.
- **Extension always follows the format.** The backend owns this logic:
  - `tarpack_set_format` rewrites the extension of a set output path, replacing
    a known archive suffix, or appending one if there is none.
  - `tarpack_set_output(path)`:
    - If `path` ends with the current format's extension, it is kept.
    - If it ends with a different known archive suffix, the format switches to
      match. The user typed that name explicitly, and the switch shows in the
      session.
    - Otherwise the current extension is appended.
  - The session supplies `suggestedOutputName`: the file name for the Save
    dialog, with the current format's extension. It comes from `output_name`
    with its suffix normalised, falling back to the manifest file stem.
  - The session also supplies the list of formats with their extensions, for
    the Save dialog filter and the format picker.
- **Extraction command.** `BuildSummary.extractCommand` gives the exact command
  for the built file, e.g. `tar --zstd --no-overwrite-dir -xpPf gateway.tar.zst`
  (see §6.2 for `--no-overwrite-dir`). `docs/tarpack-manifest.md` documents the
  command for every format.

### 3.5 Extraction on the target

| Format | Command |
| --- | --- |
| `.tar` | `tar --no-overwrite-dir -xpPf <file>` |
| `.tar.gz` | `tar -z --no-overwrite-dir -xpPf <file>` |
| `.tar.zst` | `tar --zstd --no-overwrite-dir -xpPf <file>` (GNU tar ≥ 1.31 with `zstd` installed) |
| `.tar.xz` | `tar -J --no-overwrite-dir -xpPf <file>` |

- `-P` (`--absolute-names`) is required. Without it, GNU and BusyBox tar strip
  the leading `/` and extract relative to the current directory.
- `-p` applies the archive's modes. Run as root, GNU tar also applies the
  owners.
- GNU tar also auto-detects compression when reading a file, but the displayed
  command names the decompressor explicitly, so that it also works when piped.

## 4. Milestones

Each milestone is specified in full in its task plan. The task plan is the only
plan its `implementer` reads, so any change to a milestone is made there, and
also here when it changes the index below.

| # | Task plan | Goal | Depends on |
| --- | --- | --- | --- |
| M1 | [`M1-foundation.md`](../tasks/tarpack/M1-foundation.md) | Stack, layout, boundary, commands, CI, `CLAUDE.md` | none |
| M2 | [`M2-core-state-store.md`](../tasks/tarpack/M2-core-state-store.md) | `fm-core` app dirs, namespaced state store, atomic write | M1 |
| M3 | [`M3-manifest.md`](../tasks/tarpack/M3-manifest.md) | Manifest model, parsing, validation, `ArchiveFormat`; `docs/tarpack-manifest.md` | M1 |
| M4 | [`M4-archive-writer.md`](../tasks/tarpack/M4-archive-writer.md) | Atomic, verified writer: absolute names, four formats, EOL normalisation | M2, M3 |
| M5 | [`M5-source-matching.md`](../tasks/tarpack/M5-source-matching.md) | Drop/pick matching, remembered locations and format | M2, M3 |
| M6 | [`M6-tauri-contract.md`](../tasks/tarpack/M6-tauri-contract.md) | Tauri commands, events, watcher, typed `lib/` client | M4, M5 |
| M7 | [`M7-windows-packaging.md`](../tasks/tarpack/M7-windows-packaging.md) | Portable exe, static CRT, CI artifact, end-to-end check (incl. the WebView2 middle-truncation check moved from U6) | M6, U6 |

M2 and M3 can run in parallel, and so can M4 and M5.

M3 landed in f04c008 with review gaps. Its task plan now ends with a "Review
follow-up" section. That follow-up runs as its own implementer run on the same
task plan, and lands **before M4 and M5 start**: it changes `EntryView` and
`ManifestView` (regenerating `lib/generated/`) and the parse pipeline both
depend on. It landed in f9fc7b2.

M3's task plan then gained a **"Partial results follow-up"** (P1–P8, §3.2.1),
another separate implementer run on the same task plan. It also lands
**before M4 and M5 start**: it changes the signatures of `parse` and `load`,
adds `ParseReport` (which M4's plan builder takes whole) and
`Manifest::is_complete()`, and adds the boundary type `EntryFailure` that M4's
`BuildSummary` and M6's session carry. M4's plan builder takes the
`ParseReport`, refuses only when no entry passed (`PlanError::NoEntries`),
and returns the failures in `BuildSummary`; M5 gains
`RememberedState::restore_all`; M6's session gains the failure fields and
drops the error block from `canBuild`. Those are in their task plans.

The build-with-errors decision (decisions 37–41) arrived before any of these
landed, so it changes plans only: M3's follow-up (wording in P4, P6, P7), M4,
and M6. M5 is unaffected. The UI changes it causes are listed in §5 for the
ui-designer.

M5 was implemented (WIP commit 662099b plus an uncommitted working tree) and
the review requested changes. Its task plan now ends with a **"Review
follow-up"** (R1–R8, decisions 44–47), run as its own implementer run on the
same task plan. It lands **before M6 starts**: it changes the boundary type
`DropOutcome` (typed `UnmatchedReason`, display-only paths) that M6 returns
from `tarpack_assign_dropped` and U4 renders, and it regenerates
`lib/generated/`. U4 is updated to match. The follow-up landed in 88273bd,
the second-review fixes in de7ae19, and the reviewer approved M5 at de7ae19.

M6 was implemented at 17678e4 and the reviewer approved it with low-severity
findings. Its task plan's contract wording was corrected to match what
landed (decision 48), and it now ends with a **"Review follow-up"** (R1–R7,
decisions 49–51), run as its own implementer run on the same task plan. The
follow-up changes no boundary shape: `TarpackSession` and the 17
`TarpackErrorKind`s are unchanged, and only the `stateWarning` doc comment
is regenerated. The UI tasks (U2–U6) can therefore start from 17678e4 in
parallel with it; U2 and U5 carry the corrected wording (progress phases
start over, not necessarily at 0; `ManifestChangedOnDisk` also covers an
unreadable manifest at build time; `stateWarning` may report a recent
manifest that could not be reopened). The follow-up landed in 909b36b, and the reviewer approved M6 at 909b36b.

M7 was implemented at 908abc4 and the review returned "changes required":
the exe would still import the dynamic UCRT (tauri-build's default
`staticVCRuntime`, §2.5), plus documentation and CI gaps. Its task plan now
ends with a **"Review follow-up"** (R1–R8, decisions 52–53), run as its own
implementer run on the same task plan. It changes configuration, CI, and
documentation only, no boundary type. The user runs the manual checklist
(`docs/tarpack-e2e.md`, steps 1–12) after it lands and the Windows CI job is
green.

## 5. Handoff to ui-designer

The UI is written in [`tarpack-ui.md`](tarpack-ui.md). Its constraints are:

- The UI renders `TarpackSession` and never keeps its own copy.
- It calls only `lib/` wrappers.
- Build is disabled with a stated reason unless at least one entry passed
  validation, every passed entry is `Ready`, and an output path is set.
  Manifest errors alone do not disable it (decision 37).
- Overwriting an existing output needs an explicit confirmation.
- A drop shows its matched, unmatched, and ambiguous results. Each unmatched
  item has a typed reason, and each reason has its own copy (decision 44).
- **New from the §6 answers:**
  - The user picks the output format (four options) before building. The UI
    takes the options and extensions from the session and does no extension
    logic of its own.
  - The Save dialog uses `suggestedOutputName` and a filter for the current
    format.
  - The build result shows the extraction command, including `-P`, with a way
    to copy it.
  - Entries with `normalizeEol` are visibly marked.
  - Progress has a `verifying` phase after writing.
- **New from the partial-results decision (§3.2.1):**
  - A manifest with errors still shows its passed entries.
  - Failed entries are listed in a report, each with all of its errors.
  - Manifest-level errors are shown, and when one withholds every entry the
    UI says so rather than showing an empty table.
  - The user is notified that there were errors whenever `errorCount > 0`.
- **New from the build-with-errors decision (decisions 37–41), replacing
  "build stays blocked until the error count is 0":**
  - `buildBlockedReason` no longer has `"manifestInvalid"`. It gains
    `"noEntries"`, first after `"noManifest"`, for "no entry passed" (entries
    withheld, all failed, or none listed). `TarpackErrorKind`
    `ManifestInvalid` is replaced by `NoEntries`; there are still 17 kinds.
  - With errors and at least one passed entry, the build bar enables
    **Create archive** once the passed entries are ready and an output is
    set, and states beforehand how many entries will be left out and that
    the errors will be listed in the result. No extra confirmation dialog.
  - The build result is the final report: besides what it shows today, it
    lists every left-out entry with its errors, every manifest-level error,
    and the warnings, from `BuildSummary`, and says plainly when the archive
    is missing files the manifest lists.

  The full list of contract changes is in §6.3.

The ordering is:

- UI task 1 needs only M1.
- UI tasks 2–5 need M6. The format picker and extraction command belong to the
  build bar and result (U5), which consume the M6 contract.
- M7 needs UI task 6.

## 6. Decisions

### 6.1 Answers to the open questions (resolved 2026-09-29)

| # | Question | Decision |
| --- | --- | --- |
| Q1 | UI technology | Tauri v2 + React/TypeScript/Vite, as proposed. §2.1. |
| Q2 | Archive paths | **Absolute** `/opt/...` names stored in the archive, overriding the relative proposal. The target extracts with GNU tar `-P`. The `tar` crate's `set_path` rejects absolute paths, so M4 writes header name bytes directly, with GNU long-name records for names of 100 bytes or more, and tests that stored names begin with `/`. The extraction command is documented in `docs/tarpack-manifest.md`. §3.3, §3.5. |
| Q3 | Compression | Four formats chosen at build time: `.tar`, `.tar.gz` (flate2), `.tar.zst` (zstd), `.tar.xz` (xz2/liblzma). Parameters bound decompression memory on armv7: zstd 19 with `window_log` 23, xz preset 6, gzip 6. The Save extension follows the format. The last format is remembered per manifest. §3.4. |
| Q4 | Owners | Numeric uid/gid plus names per file, default `0`/`0` `root:root`. Accepted. |
| Q5 | Manifest location | Open from anywhere; default folder `%APPDATA%\FileManager\tarpack\manifests\`. Accepted. |
| Q6 | Line endings | Opt-in per-file `normalize_eol = true` (CRLF→LF). Default is a byte-for-byte copy. §3.1, §3.3. |
| Q7 | mtime | The source file's modification time. Accepted. |
| Q8 | Distribution | Portable `.exe`, no installer. §2.5. |
| — | Consequence | zstd and liblzma build C code: Windows CI and packaging need MSVC (M1, M7), and Linux CI needs a C compiler (M1). |

### 6.2 Planner decisions arising from the answers *(proposed)*

These follow from the answers above. They stand unless the human overrides
them.

1. **`liblzma` crate rather than `xz2`.** `liblzma` is the maintained fork with
   the same API. `xz2` has been unmaintained since 2022. M4 may fall back to
   `xz2` only if `liblzma` fails to build on either CI job, and must say so in
   its handoff.
2. **`ArchiveFormat` lives in `fm_tarpack::format` and is created by M3.**
   This keeps M4 and M5 parallel.
3. **`tarpack_set_output` switches the format** when the chosen file name
   carries a different known archive suffix, instead of appending a second
   suffix or rejecting the name.
4. **Default format** comes from the remembered choice, then `output_name`'s
   suffix, then `.tar`. There is no new manifest key.
5. **`--no-overwrite-dir` in the documented and displayed extraction command.**
   The archive carries explicit directory entries such as `/etc/` and `/opt/`.
   With `-p`, GNU tar by default re-applies the mode and, as root, the owner of
   directories that already exist. A manifest whose defaults are not
   `0755 root:root` would therefore alter `/etc` or `/opt` on the target.
   `--no-overwrite-dir` keeps the metadata of existing directories and still
   applies `dir_mode` to directories the archive creates. The alternative is to
   stop emitting directory entries for paths that are likely to exist, which is
   guesswork. The flag is GNU-only, which matches the stated target.
6. **Progress gains a phase** (`writing` or `verifying`). Verification
   decompresses and re-reads the whole archive, and for xz that is long enough
   to need visible progress.
7. **`BuildSummary` reports `normalizedEntries`**: each normalised entry's id
   and the number of CRLF pairs replaced. The user can then see that the
   conversion happened.

Added after the ui-designer's contract review:

8. **`TarpackErrorKind` is a closed Rust enum**, generated as a TS
   string-literal union, so the UI's kind-to-message mapping is checked
   exhaustively. Its 17 kinds are listed in M6. Adding or renaming a kind is a
   UI contract change.
9. **No `bigint` at the boundary.** Every 64-bit integer field that crosses is
   annotated `#[ts(type = "number")]`, and a test fails if any generated file
   contains `bigint`. All values stay far below 2^53.
10. **`ArchiveFormatOption.filterExtension`** (`tar`, `gz`, `zst`, `xz`) comes
    from the backend, so the UI derives nothing from `extension`.
11. **No manifest:**
    - `format` is `"tar"`;
    - `formats` lists all four;
    - the output fields are `null`.
    - `setFormat` and `setOutput` fail with `NoManifest` and change nothing,
      because both are remembered per manifest.
12. **Progress end conditions:**
    - Each phase ends with exactly one event where `bytesDone == bytesTotal`
      and `entryId` is `null`.
    - `entryId` is the file entry being processed, and `null` for directory
      and long-name records.
    - There is no `finalizing` phase. M4 computes the SHA-256 during the
      verify pass, so only an fsync and a rename follow the final verifying
      event, and nothing is emitted after it.
13. **Verify failure saves nothing.** No file is written at the output path,
    and a pre-existing file there is left byte-for-byte unchanged (M4 tests
    this).
14. **Clipboard is not wrapped.** The UI uses `navigator.clipboard.writeText`
    directly: WebView2 treats `tauri.localhost` as a secure context, and Copy
    runs on a user click. There is no clipboard plugin or capability. If the
    M7 end-to-end check shows it failing in the packaged exe, a follow-up
    backend task adds `tauri-plugin-clipboard-manager` with a
    `writeClipboardText` wrapper.

Added after the M1 review (2026-09-29):

15. **The ts-rs export lives in the shell crate** and has an opt-in update
    mode (`UPDATE_GENERATED=1`). Both CI jobs run `cargo test --workspace`,
    and Linux installs the webkit2gtk dev packages. See §2.3 and §2.4.
16. **Only `tools::register` calls `invoke_handler` and `setup`**, once each.
    Tool modules contribute command fns and state constructors. See §2.2.
17. **The tauri-dependency check matches cargo's message**, not just the exit
    code. See §2.4.
18. **"Both CI jobs green on the pushed branch" stays M1's done condition.**
    The implementer cannot prove it before a push, so the implementer run ends
    at local checks and review. The orchestrator pushes and confirms, and M1 is
    not landed until then. Re-review C adds a second landing condition: a
    one-time `npm run tauri:dev` on Windows (§2.6).
19. **No bundle targets; restrictive CSP.** See §2.5 and §2.6.
20. **RTL cleanup is explicit** (`afterEach(cleanup)` in `test-setup.ts`),
    because the Vitest config keeps `globals: false`.
21. **`eslint-plugin-jsx-a11y` lands in M1**, not in U1. See §2.4.
22. **`ts-rs` is a workspace dependency used directly by each crate.**
    `fm-core` does not re-export it. See §2.3.

Added after the M1 re-review (2026-09-29):

23. **The hook test counts code lines only.** It skips lines whose trimmed
    start is `//` in every file it scans. Without that, the `//!` example in
    `tools/mod.rs` would make M6's correct code fail. See §2.2.
24. **The tauri-dependency check verifies each crate with `cargo pkgid` first.**
    See §2.4.
25. **The `TS_RS_EXPORT_DIR` backstop gets a one-time manual check in M1.**
    M1 temporarily adds a probe type with `#[ts(export)]`, confirms it lands
    in `target/ts-rs-stray/` and not in `bindings/`, then reverts.
26. **Parallel M4 and M5 regenerate, never merge.** Both append to `EXPORTERS`
    and regenerate `lib/generated/`. Whichever merges second re-runs the
    regenerate command after resolving `EXPORTERS`, and never hand-merges
    generated files.

Added after the M3 review (2026-09-29):

27. **Two-stage manifest parse.** `toml::de::DeTable::parse` first, then a
    walk that reports unknown keys and deserializes each top-level scalar,
    `[defaults]`, and each `[[file]]` separately. This attributes serde errors
    to the entry `id` (review B1) and lets unknown keys and type errors sit
    alongside validation errors (B3). TOML syntax still stops at the first.
28. **`.` segments in `dir` are errors** (B2), and **a target that is another
    entry's directory is an error** (N5). See §3.2.
29. **One entry-view shape** (N1). `EntryView` is
    `{ id, source, targetPath, mode, modeText, owner, uid, gid, normalizeEol }`.
    `mode` is the four-digit octal string (`"0755"`), `modeText` is symbolic
    (`"rwxr-xr-x"`), and `owner` is `"uname:gname"`. The numeric `uid`/`gid`
    also cross, because they are what the header carries, and the UI may show
    them. `ManifestView` is `{ name, outputName, entries }`, with **no**
    `defaultFormat`: the UI's format is always `session.format`.
    `outputName` stays, because the session's `manifest.outputName` comes from
    it. M6 does not embed `ManifestView` whole. Its session entry type
    flattens `EntryView` (`#[serde(flatten)]`) and adds `assigned` and
    `status`, so the entry fields are defined once, in M3.
30. **No `unsafe` in `format.rs`** (N3). `with_extension` uses the cfg'd
    `OsStrExt`/`OsStringExt` APIs, with a non-UTF-8 test per platform.

Added after the partial-results decision and the M3 re-review (2026-09-29):

31. **Partial results** (human's decision; planner's rules in §3.2.1).
    `parse` returns a `ParseReport` with the passed entries, one
    `EntryFailure` per failed table, manifest-level errors, and warnings.
    `LoadError::Invalid` is removed: a readable file always loads.
32. **An entry passes only when no error is attributed to it; cross-entry
    errors fail every entry involved** and are reported on each. The
    alternative, excluding only the later entry of a duplicate, would attach
    a remembered file by id to whichever table came first.
33. **Withholding errors.** Syntax, UTF-8, `version`, `[defaults]`, unknown
    top-level keys, and `file` not an array hide every entry; `name` and
    `output_name` errors do not.
34. ~~**Build is blocked while any error exists.**~~ **Superseded by
    decision 37** (the human, 2026-09-29). `is_complete` stays in M3 as a
    description only; `PlanError::ManifestIncomplete`, the `canBuild` error
    block, `"manifestInvalid"`, and the `ManifestInvalid` kind are dropped.
35. **Assignments work while errors exist, and failed entries' remembered
    sources are kept** (M5 `restore_all`; M6 prunes only when the manifest
    is valid).
36. **An empty `id` is attributed like a missing one** (`[[file]] #<n>: `)
    for every error from its table, `id must not be empty` included (M3
    re-review).

Added after the human overruled decision 34 (2026-09-29):

37. **Errors do not block building** (the human: *"A single error does not
    block builds but is included as an error in the final report."*). The
    archive holds the passed entries; every failed entry and every
    manifest-level error is an error in the build's final report, and
    warnings are reported too. Reason for the old rule, now answered: it
    blocked because a partial archive would silently drop files. Reporting
    every dropped entry in the result of the build that dropped it, and
    announcing the count before the build, makes the drop visible instead of
    silent, which is what the invariant requires. The human prefers a usable
    test build over a blocked one.
38. **"Nothing to build" is the only error-related block.** Zero passed
    entries (withheld, all failed, or none listed) gives
    `buildBlockedReason: "noEntries"`, `TarpackErrorKind::NoEntries`
    (replacing `ManifestInvalid`, so the union keeps 17 kinds), and
    `PlanError::NoEntries`. A valid manifest with no files is included: an
    empty archive is never what the user wants, and one rule is simpler for
    the UI than two. Order of reasons: `noManifest`, `noEntries`,
    `entriesNotReady`, `noOutput`.
39. **The report is carried by the library, not assembled by the shell.**
    `ArchivePlan::new(&ParseReport, &Assignments)` copies the failures,
    manifest-level errors, and warnings into the plan, and `BuildSummary`
    returns them: `builtIds: string[]`, `leftOut: EntryFailure[]`,
    `manifestErrors: Diagnostic[]`, `warnings: Diagnostic[]`, and
    `errorCount: number`. `EntryFailure` and `Diagnostic` are M3's types,
    reused, not redefined. Alternative rejected: keep `ArchivePlan` on a bare
    `Manifest` and let M6 attach the failures. That leaves a library path
    that builds a partial archive with no record of what it left out.
40. **No extra confirmation for building with errors.** The build bar states
    beforehand how many entries will be left out and that the errors will be
    listed in the result; that, plus the report, is enough. A modal on every
    test build of a manifest being fixed would be dismissed by habit.
41. **The report goes to the UI only.** It is not written into the archive
    (no metadata entry, no comment) and not into a sidecar or log file. The
    archive's content must be exactly the manifest's passed entries and
    their directories, and a file beside the output is one the user did not
    ask for. If a persisted report is wanted later, it is a new decision.

Added after the M4 review (2026-09-29):

42. **M6 coalesces build progress; M4 does not throttle.** As landed,
    `write_archive` calls `progress` once per ~8 KiB source read while
    writing and once per 64 KiB while verifying, so a 1 GB entry gives about
    130,000 + 16,000 events. Emitting each as a Tauri event would flood the
    IPC channel and the React render loop. `tarpack_build` feeds M4's events
    to a `ProgressCoalescer` (a plain struct in `tools/tarpack`, clock passed
    in) and emits only what it forwards: the first event of each phase, the
    first event of each file entry in both phases, each phase's final event
    (`bytes_done == bytes_total`, `entry_id: None`), and otherwise at most one
    event per 50 ms. It forwards a subsequence of M4's events, unchanged and
    in order, never holding one back or flushing later, so `bytesDone` stays
    monotonic per phase, both final events arrive exactly once, and nothing
    follows the final verifying event (decision 12 holds unchanged).
    Alternatives rejected: throttling in `fm-tarpack` (M4 has landed with
    tests pinning its behaviour, and the right rate is a UI-transport
    concern, not a domain one); throttling in `lib/tarpack.ts` (the flood
    would already have crossed IPC); a trailing-flush timer (needs a thread,
    and risks an event after the final one). Per-entry forwarding is bounded
    by the manifest's entry count, which is hand-written and small.
43. **`BuildSummary.path` and `extractCommand` are display strings, one way
    only.** `path` is a lossy conversion of the output `PathBuf`, and
    `extractCommand` is shell text for the target. Neither is ever parsed
    back into a path, by Rust or by the UI. `tarpack_reveal_output` takes no
    argument and reveals the `PathBuf` the last successful build wrote, kept
    in M6's state; with no successful build in the session it fails with
    `NoOutput` (the kind already exists; the union still has 17 kinds). This
    keeps the "paths are the platform's path type" invariant for names that
    are not valid Unicode.

Added after the M5 review (2026-09-30):

44. **Unmatched reasons are a closed enum, not English text.**
    `DropOutcome.unmatched: Vec<Unmatched { path, reason: UnmatchedReason }>`
    with `UnmatchedReason { AlreadyAssigned, NoEntry, NotFound,
    LinkNotFollowed, FolderNoMatch, Unreadable, NotUnicode }`, camelCase
    serde, exported through ts-rs. Copy belongs to the UI, which maps every
    reason in an exhaustive `Record`, so a new reason fails the typecheck
    instead of leaking a Rust string to the user. The same pattern as
    `TarpackErrorKind` (decision 8).
45. **Paths that are not valid Unicode are an accepted limit: rejected at
    the drop boundary, reported, never stored or sent as paths.** serde
    serializes a `PathBuf` only when it is Unicode, so one such path would
    make every later `Store::save` fail, or fail a whole `DropOutcome` over
    IPC, and a lossy state key could merge two manifests. Every path the UI
    sends (drops, Browse, Save) arrives as a JSON string and is already
    Unicode, so the only source is a folder walk. There, a file that would
    be assigned but whose path is not Unicode goes to `unmatched` with
    `NotUnicode`, and `matched` holds only Unicode paths. `Unmatched.path`
    and `Ambiguity.candidates` are display-only and serialize lossily (one
    way, as in decision 43); the outcome types derive `Serialize` only.
    `RememberedState`'s key uses `to_str()` (no memory for a non-Unicode
    manifest path), and `remember` / `touch_recent` skip non-Unicode paths
    as defence in depth. `Assignments` still accepts any path, since the
    writer can read any file. Rejected: a lossless encoding (for example
    WTF-16 units as a number array in the state file and over IPC). It adds
    a second path representation to the store, the contract, and the UI for
    names Windows tools rarely produce and that the user can fix by
    renaming, and the drop result tells them to.
46. **Drops report every item that did not apply.** A walked file that fits
    only `Ready` entries is `AlreadyAssigned` (a folder is `FolderNoMatch`
    only if no file in it fits any entry); unreadable directories and
    children met in the walk are `Unreadable`, while other matches still
    apply; a path dropped directly keeps its direct report even when a
    dropped folder also contains it. Reason: a silent skip could hide a
    second candidate and turn an ambiguity into a guess, or leave the user
    with an empty result and no explanation.
47. **Implementer choices accepted in the M5 review.** The depth cap counts
    the dropped folder's direct children as level 1 (level 8 found, level 9
    not); `remember` replaces the stored sources, which is how pruning
    happens; the state key falls back to the path as given when
    `canonicalize` fails; `is_symlink` also detects junctions on Windows, so
    the walk needs no separate junction check. The Windows key strips
    `\\?\UNC\` to `\\` and `\\?\` to nothing, then lowercases.

Added after the M6 review (2026-09-30):

48. **Contract wording corrected to what landed.**
    - Capabilities are exactly `core:event:allow-listen`,
      `core:event:allow-unlisten`, `dialog:allow-open`, and
      `dialog:allow-save`. The opener is called from Rust only, so the
      webview has no opener permission, and the UI never emits events.
    - Each progress phase starts over; its first event may already be past
      0 (M4 reports after the first 512-byte record) and is below the total.
      "From 0" is no longer promised.
    - `ManifestChangedOnDisk` also means the manifest can no longer be read
      at build time: what is on disk is not what the user saw either way.
    - `set_output` and `set_format` rewrite a recognised suffix to the
      canonical extension (`.tgz` becomes `.tar.gz`, case normalised).
    - `create_from_example` opens the created manifest, arms the watcher,
      and returns the session.
49. **No filesystem walk under the session lock.** `tarpack_assign_dropped`
    clones what matching needs into a `DropJob`, walks on `spawn_blocking`
    with no lock held, and applies only if a session revision counter is
    unchanged, retrying up to 3 times and otherwise failing with `Io`
    (nothing applied). This mirrors `tarpack_build`'s `BuildJob`. Rejected:
    holding the lock inside `spawn_blocking` (other commands would still
    block their async workers on the `std::sync::Mutex`), and applying a
    stale outcome filtered by id (after a reopen the ids may belong to a
    different manifest).
50. **Nothing is left or lost silently around the session.**
    `create_from_example` removes the partial file it created when the write
    fails (it never removes a file it did not create); a recent manifest
    that cannot be reopened at startup is reported in `stateWarning`, which
    now accumulates warnings instead of replacing them; `reload` persists
    `RememberedState` like every other mutating command.
51. **Capabilities narrowed to `allow-listen` / `allow-unlisten`** in place
    of `core:event:default`, which also granted emit. A test pins the list.

Added after the M7 review (2026-09-30):

52. **tauri-build's static VC runtime is off; `+crt-static` alone decides
    the CRT.** Set as `build.windows.staticVCRuntime: false` in
    `tauri.conf.json`, explained in a comment in `.cargo/config.toml`.
    Rejected: the `build.rs` form
    (`WindowsAttributes::new().static_vc_runtime(false)` via
    `tauri_build::try_build`). Both are accepted by the locked tauri-build
    2.7.0 / tauri-utils 2.10.0 and the CLI 2.12.0 schema; the config key is
    declarative, schema-checked (the struct denies unknown fields), and
    keeps `build.rs` a one-liner.
53. **The dependency check needs no third-party action and also forbids
    `WebView2Loader.dll`.** `dumpbin` is located with the runner's
    preinstalled `vswhere.exe`, replacing the unpinned
    `ilammy/msvc-dev-cmd@v1` (rejected alternative: pin it to a commit SHA,
    which still trusts a third party for one binary path). A loader DLL
    beside the exe would break the single-file portable exe, so the check
    fails on it like on the CRT, zstd, and liblzma DLLs.

### 6.3 UI-facing contract changes (for the ui-designer)

All of these are owned by M6 (with types from M3–M5) and generated into
`apps/desktop/src/lib/generated/`:

- New type `ArchiveFormat = "tar" | "tarGz" | "tarZst" | "tarXz"`.
- New type `ArchiveFormatOption = { format: ArchiveFormat, extension: string, filterExtension: string }`
  (e.g. `{ format: "tarZst", extension: ".tar.zst", filterExtension: "zst" }`).
- `TarpackSession` gains:
  - `format: ArchiveFormat`;
  - `formats: ArchiveFormatOption[]` (fixed order: tar, tarGz, tarZst, tarXz);
  - `suggestedOutputName: string | null`.

  With no manifest loaded, `format` is `"tar"`, `formats` lists all four, and
  `outputPath` and `suggestedOutputName` are `null` (decision 11).
- `TarpackSession.manifest.entries[]` gains `normalizeEol: boolean`.
- **Partial results (§3.2.1).** `TarpackSession.manifest` gains:
  - `entriesWithheld: boolean`: a manifest-level error hides every entry, and
    `entries` is `[]`;
  - `failedEntries: EntryFailure[]`, one per `[[file]]` table with errors, in
    manifest order, where
    `EntryFailure = { index: number, id: string | null, source: string | null, line: number, errors: Diagnostic[] }`
    (`index` is the 1-based table position, `line` its `[[file]]` header);
  - `errorCount: number`, the manifest-level errors plus every failed entry's
    errors; above 0 means "notify the user". It does **not** block the build
    (decision 37).

  `manifest.errors` now holds **only** manifest-level errors; entry errors
  are in `failedEntries`. `manifest.entries` holds only passed entries, and
  `readyCount` / `totalCount` count only those. `name` falls back to the
  file stem when the manifest's `name` is in error. Assign, clear, drop,
  `setFormat`, and `setOutput` keep working while errors exist; assigning a
  failed entry's id fails with `UnknownEntry`.
- **Build with errors (decisions 37–41).**
  - `canBuild` is true when a manifest is loaded, at least one entry passed,
    every passed entry is `ready`, and `outputPath` is set, whatever
    `errorCount` is.
  - `buildBlockedReason` is
    `null | "noManifest" | "noEntries" | "entriesNotReady" | "noOutput"`, the
    first failing condition in that order. `"manifestInvalid"` is gone;
    `"noEntries"` means `manifest.entries` is empty (withheld, every entry
    failed, or none listed).
  - `tarpack_build` builds the passed entries and returns the report in
    `BuildSummary` (below). With no passed entries it fails with `NoEntries`.
- Each entry is `EntryView` (from M3) plus `assigned` and `status`:
  `{ id, source, targetPath, mode, modeText, owner, uid, gid, normalizeEol, assigned, status }`.
  `mode` is `"0755"`, `modeText` is `"rwxr-xr-x"`, and `owner` is
  `"root:root"` (`uname:gname`). `uid` and `gid` are numbers (decision 29).
- `outputPath` is always normalised to end with the current format's extension.
- New command `tarpack_set_format(format)` returns `TarpackSession`, and
  `lib/tarpack.ts` gains `setFormat(format)`.
- `tarpack_set_output(path)` may change `format` (decision 6.2.3) and may
  append an extension.
- `setFormat` and `setOutput` reject with `NoManifest` when no manifest is
  loaded, and change nothing.
- `BuildSummary` gains:
  - `format: ArchiveFormat`;
  - `uncompressedBytes: number`;
  - `extractCommand: string`;
  - `normalizedEntries: { id: string, crlfReplaced: number }[]`.

  `bytes` is now the size of the output file on disk. Every numeric field
  (`entries`, `files`, `dirs`, `bytes`, `uncompressedBytes`, `crlfReplaced`,
  `errorCount`) is TS `number`, never `bigint` (decision 9).
- `BuildSummary` also carries the final report (decision 39):
  - `builtIds: string[]`: the ids of the file entries in the archive, in
    manifest order (every passed entry);
  - `leftOut: EntryFailure[]`: every failed entry, none of which is in the
    archive, each with all its errors (the same records as the session's
    `failedEntries` when the build started);
  - `manifestErrors: Diagnostic[]`: the manifest-level errors (entries are
    never withheld in a build that ran, so these are the non-withholding
    ones: `name`, `output_name`);
  - `warnings: Diagnostic[]`;
  - `errorCount: number`: `manifestErrors.length` plus every `leftOut[i]`'s
    errors; 0 means the archive holds everything the manifest lists.
- The `tarpack://build-progress` payload becomes
  `{ phase: "writing" | "verifying", entryId: string | null, bytesDone: number, bytesTotal: number }`.
  - The byte counts are uncompressed tar-stream bytes, so they are comparable
    across formats. Both phases share `bytesTotal`.
  - `entryId` is the file being processed, and `null` for directory and
    long-name records.
  - Each phase ends with one event where `bytesDone == bytesTotal` and
    `entryId: null`.
  - No event follows the final verifying event, and there is no `finalizing`
    phase. The UI's "Finishing…" state covers the gap until `build` settles
    (decision 12).
  - Events are coalesced by M6 (decision 42): at most one per 50 ms, plus the
    first event of each phase, the first event of each file entry, and each
    phase's final event. The UI must not assume one event per chunk or a
    fixed rate; every rule above still holds. U5 needs no change for this.
- `BuildSummary.path` and `extractCommand` are for display and copying only,
  never parsed back into paths; `revealOutput()` takes no argument and
  reveals the path Rust kept from the last successful build (decision 43).
- `TarpackError` is `{ kind: TarpackErrorKind, message: string, entryId?: string }`.
  `TarpackErrorKind` is a closed string-literal union (decision 8):
  `"NoManifest" | "ManifestUnreadable" | "NoEntries" | "ManifestChangedOnDisk" | "UnknownEntry" | "NotAFile" | "NoOutput" | "EntriesNotReady" | "OutputExists" | "PathExists" | "SourceMissing" | "SourceUnreadable" | "SourceChanged" | "VerifyFailed" | "BuildInProgress" | "OpenerFailed" | "Io"`.
  Newly relevant to the UI copy:
  - `NoEntries` (replaces `ManifestInvalid`, decision 38): there is no entry
    that can be built, because every entry is withheld or failed, or the
    manifest lists none. Defensive: `canBuild` already blocks it.
  - `SourceChanged`: a source's size changed during the build.
  - `VerifyFailed`: the archive failed its post-write check. Nothing was saved,
    and any existing file is unchanged (decision 13).
- **Drop outcome (decisions 44–46).** `DropOutcome` is
  `{ matched: [string, string][], unmatched: Unmatched[], ambiguous: Ambiguity[] }`
  with `Unmatched = { path: string, reason: UnmatchedReason }`,
  `UnmatchedReason = "alreadyAssigned" | "noEntry" | "notFound" | "linkNotFollowed" | "folderNoMatch" | "unreadable" | "notUnicode"`,
  and `Ambiguity = { id: string, candidates: string[] }`. The paths in
  `unmatched` and `candidates` are display strings (U+FFFD where a path was
  not Unicode) and are never passed back. U4 maps each reason to copy.
- Clipboard: no `lib/` wrapper. The UI uses `navigator.clipboard.writeText`
  (decision 14).
- `lib/tauri.ts`: `saveFileDialog({ defaultPath, filters? })` gains `filters`.
- `Diagnostic` messages change wording for the long-name warning ("100 bytes or
  longer, stored path including the leading /"). The shape is unchanged.
