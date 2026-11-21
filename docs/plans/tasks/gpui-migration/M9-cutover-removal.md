# M9 — Cutover: remove Tauri, React, npm and ts-rs; the GPUI app becomes `filemanager`

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: M8 (landed)

## Goal

Make the GPUI app the only FileManager: delete the Tauri shell and the
React frontend, npm and ts-rs, rename the package and binary to
`filemanager`, and reduce CI to the Rust-only pipeline, keeping the
portable exe, its static C runtime and the dumpbin check.

## Context

FileManager moved its UI to GPUI in `apps/filemanager` (package
`filemanager-gpui`, lib `fm_app`, binary `filemanager-gpui`; harness
binary `filemanager-shoot` behind feature `shoot`). Every tool's behaviour
is in its model (`apps/filemanager/src/tools/<tool>/`), the views and
screenshots are done, and the manual's images come from the Rust harness.
The Tauri app in `apps/desktop/` still builds and ships; this milestone
removes it.

**Precondition:** `docs/gpui-test-ledger.md` has no row with status `open`.
If any is open, stop and report the rows; do not start.

What goes:

- `apps/desktop/` entirely: `src-tauri/` (the Tauri shell, `tools/*/mod.rs`
  command glue, `generated_types.rs`, `tauri.conf.json`,
  `capabilities/default.json`, `icons/`, `build.rs`), `src/` (React views,
  `lib/*.ts`, `lib/generated/`, styles, tests), `package.json`,
  `vite.config.ts`, `tsconfig.json`, `.oxlintrc.json`, `index.html`.
  `apps/filemanager/assets/icon.ico` already holds the icon.
- Root `package.json` and `package-lock.json`; `node_modules/` is ignored
  and not tracked (remove it from `.gitignore` only if nothing else needs
  the entry).
- `.gitattributes`'s `apps/desktop/src/lib/generated/** text eol=lf` line
  (delete the file if it becomes empty).
- ts-rs: the `ts-rs` entry in `[workspace.dependencies]`, the dependency in
  every crate, every `#[derive(… TS …)]` and `#[ts(…)]` attribute (about 30
  files under `crates/`), and `TS_RS_EXPORT_DIR` with its comment in
  `.cargo/config.toml`.
- serde derives that existed only to send a type over Tauri IPC, together
  with tests whose only purpose was the JSON shape seen by TypeScript
  (camelCase keys, `skip_serializing_if`, string unions). **Keep** serde
  where persistence or parsing needs it: `fm_core::Store` state
  (`RememberedState`s), the TOML manifest and config models, and anything
  else read from or written to disk. List every removed derive group and
  removed test in the PR with its reason.
- From `.cargo/config.toml`, the comment about `staticVCRuntime` in
  `tauri.conf.json` (keep `+crt-static` and the warning about `RUSTFLAGS`).

What changes:

- `apps/filemanager/Cargo.toml`: package `filemanager`, binary
  `filemanager` (lib stays `fm_app`); the root workspace `members` drop
  `apps/desktop/src-tauri`. `--self-check`, `filemanager-shoot` and the
  feature `shoot` are unchanged.
- CI (`.github/workflows/ci.yml`):
  - linux: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
    `cargo test --workspace --locked`, the five-crate dependency check;
    remove `actions/setup-node`, `npm ci`, the frontend build, typecheck,
    lint, test, and the webkit2gtk apt packages.
  - windows (`windows-2025`): clippy (with and without `--features shoot`),
    `cargo test --workspace --locked`, `cargo build --release -p filemanager --locked`,
    the dumpbin check on `target\release\filemanager.exe` (forbidden list
    unchanged: `vcruntime*`, `msvcp*`, `api-ms-win-crt-*`, `zstd*`,
    `liblzma*`, `sqlite3*`, `WebView2Loader.dll`), the self-check, the
    release-graph `test-support` check and the "no `filemanager-shoot.exe`
    in release" check, the harness runs and upload, and the artifact
    `FileManager-<version>-x64.exe` with the version from
    `cargo metadata --format-version 1 --no-deps` (package `filemanager`),
    no longer from `tauri.conf.json`. Remove the second (gpui) artifact.
  - The five-crate dependency check keeps forbidding `gpui-pre`,
    `gpui-pre-platform` and `gpui-component`; drop `tauri` from its list
    (no package of that name exists any more).
- Docs that would be wrong after this milestone, minimally: `README.md`
  (stack sentence, where the exe comes from: CI artifact or
  `cargo build --release -p filemanager`), `CLAUDE.md` **Status**, **Layout**
  and **Commands** (no npm, no Tauri, no `lib/generated`, no regenerate
  command; the commands are `cargo fmt --all --check`, clippy, tests, the
  release build, `cargo run -p filemanager`, the harness command). The
  invariants, "Adding a tool", the agent roles and the other docs are the
  next milestone's; leave a one-line note at the top of `CLAUDE.md`'s
  Invariants section: "Being rewritten for GPUI (gpui-migration M10);
  rules that name Tauri, ts-rs or `lib/` no longer apply."

### Rules that bind this task

- The portable exe stays: static CRT, no forbidden DLL, one file, no
  installer.
- The `fm-*` crates gain no dependency; they lose ts-rs (and IPC-only
  serde).
- Persisted state stays readable: do not change the serialised form of any
  type `fm_core::Store` writes (a test that loads a state file written by
  the previous build must still pass).
- The release exe never contains `test-support` or the harness.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.
- Tests touching the filesystem use temp dirs.

## Files

Delete: `apps/desktop/`, `package.json`, `package-lock.json`, the
`.gitattributes` line.

Edit: `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`,
`crates/*/Cargo.toml` and the `crates/**` files with ts-rs or IPC-only serde,
`apps/filemanager/Cargo.toml`, `.github/workflows/ci.yml`, `README.md`,
`CLAUDE.md` (sections named above), `.gitignore` if needed.

Do not touch: `docs/plans/`, the agent roles, `DESIGN.md`, `PRODUCT.md`,
`docs/pipeline.md`, `docs/schedule.md` (next milestone).

## Acceptance criteria

- `git ls-files` lists no `package.json`, `package-lock.json`, `*.ts`,
  `*.tsx`, `*.js` (outside `.github/` if any), `*.css`, `tauri.conf.json`.
- `cargo tree --workspace --locked -i ts-rs` and `-i tauri` match no
  package; `grep -rn "ts_rs\|#\[ts(" crates apps` finds nothing.
- `cargo build --release -p filemanager --locked` produces
  `target\release\filemanager.exe`; dumpbin passes; `--self-check` from a
  folder holding only the exe exits 0; the artifact is named
  `FileManager-<version>-x64.exe` with the workspace version.
- Both CI jobs are green with no Node step.
- Persisted state of the previous build loads (test below).
- `CLAUDE.md` Status, Layout and Commands describe the GPUI app only.

## Tests proving completion

- Every existing Rust test still passes (`cargo test --workspace`), minus
  the JSON-shape tests removed with their reasons listed in the PR.
- `fm_tarpack` and `fm_dbmanager`: `remembered_state_from_previous_build_loads`
  (a state file's text captured before this milestone, committed as a test
  fixture string, loads through `Store` unchanged). Add the same for any
  other tool with persisted state.
- CI's dumpbin, self-check, release-graph and harness steps.

## Out of scope

- The invariant rewrite, agent roles, `DESIGN.md`/`PRODUCT.md`,
  `docs/pipeline.md`, `docs/schedule.md`, `docs/plans/README.md` (next
  milestone).

## Risks

- Removing serde from a type that `Store` persists would break saved state:
  check every `Store<…>` type before removing a derive.
- A crate that used `serde_json` only for IPC-shape tests may no longer need
  it as a dev-dependency; remove unused dependencies (clippy does not flag
  them; check with `cargo udeps` if available, or by hand).
- CI time drops (one release exe instead of two).
