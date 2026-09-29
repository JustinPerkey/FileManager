# M1 — Foundation: workspace, Tauri shell, tool registry, CI

Status: implemented, reviewer approved (after two rework rounds; see "Review
rework" below). Awaiting the landing conditions: both CI jobs green on the
pushed branch, and one `npm run tauri:dev` on Windows to confirm `devCsp`.
Project: tarpack   Depends on: none

## Goal

Establish the stack, the repository layout, the core/UI boundary, the
verification commands, and CI, and record all of it in `CLAUDE.md`, so that
every later task builds on a stated foundation.

## Review rework

A first implementation of this task is already in the working tree
(uncommitted). The reviewer asked for changes. **This plan describes the
required end state in full.** Bring the working tree to it. The changes from
the first implementation, keyed to the review findings:

| Finding | What changes |
| --- | --- |
| P1-1 | The export test gains an update mode that rewrites `lib/generated/`. The command goes in `CLAUDE.md`. `.cargo/config.toml` points `TS_RS_EXPORT_DIR` at `target/` so a stray `#[ts(export)]` cannot write into a crate. |
| P1-2 | The export test moves out of `crates/fm-tarpack/tests/generated_types.rs` into the shell crate, as `apps/desktop/src-tauri/src/generated_types.rs`. Both CI jobs run `cargo test --workspace` and Linux installs the webkit2gtk dev packages. |
| P2-3 | Only `tools::register` may call `invoke_handler` and `setup`, each once. Tool modules contribute only command fns and managed-state constructors. `lib.rs` never calls `invoke_handler`. A test enforces this. |
| P2-4 | The CI tauri-dependency check passes only on cargo's "did not match any packages" message. Any other failure fails the step. |
| P2-5 | Kept: both CI jobs green on the pushed branch is the done condition (see "Done condition"). |
| P3-6 | `tauri.conf.json` loses `bundle.targets`. |
| P3-7 | `tauri.conf.json` gets a restrictive CSP instead of `null`. |
| P3-8 | `src/test-setup.ts` registers `afterEach(cleanup)`. |
| P3-9 | `eslint-plugin-jsx-a11y` lands in this task. |
| P3-10 | The `CLAUDE.md` Layout gains the missing files. `ts-rs` becomes a workspace dependency used directly by each crate. `fm-core` drops `ts-rs` and its `pub use ts_rs` re-export. |

**Re-review (round 2).** The working tree already has most of the above.
Close these as well:

| Item | What changes |
| --- | --- |
| A (P2) | `builder_hooks_only_in_tool_registry` counts `.invoke_handler(` and `.setup(` in comments too. The `//!` example in `tools/mod.rs` already has one of each, so M6's real calls would push each count to 2 and fail correct code. The scan must skip every line whose trimmed start is `//`, in every file it scans, and the test's doc comment must say so. See "Enforcement". |
| B (P3) | With `-i`, `cargo tree` ignores a `-p` that matches nothing. It exits 0 and prints the whole inverse tree, so a misspelled crate fails as "depends on tauri", not "unexpected". The CI step gains a `cargo pkgid` existence check first. See "The tauri-dependency check". |
| C (P3) | `devCsp` cannot be tested in this container. A one-time `npm run tauri:dev` on Windows becomes part of M1 landing, done by the human or orchestrator. See "Done condition". |
| D (P3) | A one-time check that the `TS_RS_EXPORT_DIR` backstop works. See "Acceptance criteria". |

## Context

**The product.** FileManager is a Rust desktop application that runs only on
Windows. It will hold several independent tools ("subcomponents"). The first
tool is the Tar Packager (`tarpack`), which builds a Linux `.tar` from Windows
files. This task builds no tool logic. It builds the frame every tool plugs
into.

**Stack** (decided by the human):

- A Tauri v2 shell.
- Rust workspace crates for all domain logic.
- A React + TypeScript + Vite frontend.
- npm workspaces at the root.

**Distribution** (decided by the human): a **portable `.exe`**, with no
installer. `npm run tauri:build` runs `tauri build --no-bundle`. The exe lands
in the workspace target directory (`target/release/filemanager.exe`). M7 later
adds static CRT linking and the CI artifact upload. This task only makes the
unbundled build work. `tauri.conf.json` therefore names no bundle targets:
`bundle.active` is `false` and there is **no `targets` key**. Keep
`bundle.icon`, because the Windows build embeds `icon.ico` in the exe.

**Native build prerequisites.** Later tasks add the `zstd` and `liblzma`
crates, which compile bundled C sources. Record these in `CLAUDE.md` now, so
that the prerequisite is stated before it bites:

- Windows needs the MSVC toolchain (Visual Studio Build Tools, "Desktop
  development with C++"), which Tauri already requires.
- Linux needs a C compiler (`cc`/`gcc`).
- Building the Tauri shell on Linux (which CI now does, see below) needs the
  webkit2gtk dev packages: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`,
  `libsoup-3.0-dev`, and `libjavascriptcoregtk-4.1-dev`.

This task adds no C dependency itself.

**Layout to create.**

```
Cargo.toml                    workspace: members crates/*, apps/desktop/src-tauri;
                              [workspace.dependencies] pins ts-rs (and tempfile)
.cargo/config.toml            [env] TS_RS_EXPORT_DIR (see "Generated types")
.gitattributes                apps/desktop/src/lib/generated/** text eol=lf
rust-toolchain.toml           stable, pinned to a specific version
rustfmt.toml
package.json                  root scripts (below), npm workspaces: apps/desktop
crates/
  fm-core/                    shared, tool-agnostic library. No tauri dependency.
  fm-tarpack/                 Tar Packager domain library. Depends on fm-core only.
                              No tauri dependency.
apps/desktop/
  package.json, vite.config.ts, tsconfig.json, eslint.config.js, index.html
  src-tauri/
    Cargo.toml                package and binary name: filemanager
    build.rs                  tauri_build::build()
    tauri.conf.json
    capabilities/default.json
    icons/
    src/main.rs               calls filemanager_lib::run()
    src/lib.rs                builder, plugins, then tools::register(builder).run(...)
    src/tools/mod.rs          tool registry: the only place that calls
                              invoke_handler / setup (see below)
    src/generated_types.rs    #[cfg(test)] ts-rs export and currency test
  src/
    main.tsx, App.tsx         minimal placeholder that renders the app name
    App.test.tsx
    test-setup.ts             jest-dom matchers + afterEach(cleanup)
    lib/tauri.ts              thin wrappers over @tauri-apps/api (invoke, listen)
    lib/generated/            TS types generated from Rust (ts-rs); only .gitkeep for now
    tools/registry.ts         export const tools: ToolEntry[] = []
.github/workflows/ci.yml
.env.example                  a comment stating no configuration keys exist yet
```

Delete `crates/fm-tarpack/tests/generated_types.rs` (the first
implementation's location). `fm-tarpack` keeps `tempfile` as a dev-dependency,
switched to `tempfile = { workspace = true }`; later tasks use it.

**The core/UI boundary** falls at `apps/desktop/src/lib/`.

- The backend `implementer` owns everything in Rust and everything under
  `lib/`.
- The `ui-implementer` owns everything else under `apps/desktop/src/`.
- Types that cross the boundary are defined once in Rust with
  `#[derive(ts_rs::TS)]` and exported into `apps/desktop/src/lib/generated/`.
  No hand-written TS duplicates.

### Generated types (ts-rs)

**The `ts-rs` dependency.** Declare `ts-rs` once, in the root `Cargo.toml`
under `[workspace.dependencies]`, with the version the first implementation
already uses (12). Each crate that derives `TS` adds
`ts-rs = { workspace = true }` to its own `[dependencies]` and writes
`#[derive(ts_rs::TS)]`. The derive expands to paths under `::ts_rs`, so a
re-export from another crate is not the mechanism. In this task:

- `fm-core` has **no** `ts-rs` dependency and no `pub use ts_rs`. It has no
  boundary types yet.
- `fm-tarpack` has no `ts-rs` dependency yet. M3 adds it the same way.
- The shell crate `filemanager` has `ts-rs = { workspace = true }` under
  `[dependencies]`, because it owns the export and, from M6, its own boundary
  types. It also adds `tempfile = { workspace = true }` under
  `[dev-dependencies]`.

**Where the export lives, and why.** The export test is a unit-test module
inside the shell crate: `apps/desktop/src-tauri/src/generated_types.rs`,
declared in `src/lib.rs` as `#[cfg(test)] mod generated_types;`.

- The shell crate is the only crate that depends on every `fm-*` crate, so it
  sees every boundary type.
- It is also where M6 defines its own boundary types, in the private module
  `src/tools/tarpack.rs`. A unit-test module inside the crate can name private
  items, and an integration test under `tests/` cannot. That is why this is a
  `src/` module and not a `tests/` file.

**Contents of `generated_types.rs`:**

- `type Exporter = fn(&ts_rs::Config) -> Result<(), ts_rs::ExportError>;`
- `const EXPORTERS: &[Exporter] = &[];`, with a doc comment:
  - there is one entry per root boundary type, written
    `<path::Type as ts_rs::TS>::export_all`;
  - `export_all` also writes the types it references;
  - later tasks append to this list (fm-tarpack types from M3 onward, shell
    types from M6).
- The committed directory is
  `concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/generated")`.
- `fn export_fresh() -> tempfile::TempDir`: builds
  `ts_rs::Config::from_env().with_out_dir(tmp)` and runs every exporter into a
  fresh temp dir.
- `fn read_tree(root) -> BTreeMap<PathBuf, Vec<u8>>`: every file under `root`
  by relative path, skipping `.gitkeep`. Keep the first implementation's
  version.
- `fn sync_dir(fresh: &Path, target: &Path) -> io::Result<()>`:
  1. Remove every file and subdirectory in `target` except a top-level
     `.gitkeep`.
  2. Copy the `fresh` tree into `target`, creating subdirectories as needed.
  3. Leave `.gitkeep` in place (create it if it is missing), so the directory
     survives in git when empty.
- `#[test] fn generated_types_are_current()`:
  - Always export fresh first, so a failing export never half-clears the
    committed directory.
  - **Update mode:** when the environment variable `UPDATE_GENERATED` is `1`:
    - if the `CI` environment variable is also set, panic with "UPDATE_GENERATED
      must not be used in CI";
    - otherwise call `sync_dir(fresh, committed)`, print the number of files
      written, and pass.
  - **Check mode (default):** compare `read_tree(committed)` with
    `read_tree(fresh)`. On mismatch, fail with a message that:
    - lists the added, removed, and changed relative paths;
    - ends with the exact regenerate command (below).
  - With `EXPORTERS` empty and only `.gitkeep` committed, it passes.
- `#[test] fn sync_replaces_stale_files_and_keeps_gitkeep()`: works entirely in
  temp dirs, never on the committed directory. Build a target containing
  `.gitkeep`, a stale file, and a stale nested directory, and a fresh tree
  containing a new file and a nested file. After `sync_dir`, the target holds
  exactly `.gitkeep` plus the fresh tree, byte for byte.

**The regenerate command.** Recorded in `CLAUDE.md` Commands, in both shell
forms:

```sh
UPDATE_GENERATED=1 cargo test -p filemanager --lib generated_types_are_current
```

```powershell
$env:UPDATE_GENERATED=1; cargo test -p filemanager --lib generated_types_are_current; Remove-Item Env:UPDATE_GENERATED
```

The filter names exactly one test, so update mode never runs alongside a test
that reads the directory.

**`#[ts(export)]` is not used.** The `EXPORTERS` list is the only writer of
`lib/generated/`. The per-type `#[ts(export)]` attribute generates its own test
that writes to `TS_RS_EXPORT_DIR`, or to `./bindings` inside the crate when that
variable is unset. As a backstop, create `.cargo/config.toml` with:

```toml
[env]
# ts-rs: the only writer of apps/desktop/src/lib/generated is
# apps/desktop/src-tauri/src/generated_types.rs. A stray #[ts(export)] writes
# here, inside the ignored target dir, never into a crate.
TS_RS_EXPORT_DIR = { value = "target/ts-rs-stray", relative = true }
```

`generated_types.rs` always overrides this directory with `with_out_dir`. M7
later adds a `[target.x86_64-pc-windows-msvc]` section to this same file.

**Line endings.** `windows-latest` checks files out with CRLF by default,
while ts-rs writes LF, so the currency test would fail there. Add
`.gitattributes` with:

```
apps/desktop/src/lib/generated/** text eol=lf
```

### Tool registry pattern (later tools follow it)

**Rust.** Tauri's `Builder::invoke_handler` and `Builder::setup` each
**replace** any earlier call. If each tool called `invoke_handler` itself, a
second tool would silently drop the first tool's commands. So:

- `src-tauri/src/tools/mod.rs` exposes
  `pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry>`.
  It is the **only** place in the crate that calls `.invoke_handler(...)` or
  `.setup(...)`, and it calls each **at most once**:
  - `.manage(...)` once per tool that has state;
  - **one** `.invoke_handler(tauri::generate_handler![...])` listing every
    tool's commands (for example `tarpack::tarpack_open, tarpack::tarpack_build`);
  - **one** `.setup(...)` if any tool needs startup work, which calls each such
    tool's `setup(app)` function in turn.
- Each tool is a module `tools/<tool>.rs` (or `tools/<tool>/mod.rs`). It
  contributes only:
  - its `#[tauri::command]` functions, all named `<tool>_...`;
  - a constructor for its managed-state type, if it has state;
  - optionally a `pub(super) fn setup(app: &mut tauri::App) -> ...` if it
    needs startup work.

  A tool module never receives or returns the `Builder`.
- `src/lib.rs` registers plugins (dialog, opener), then calls
  `tools::register(builder)` and `.run(tauri::generate_context!())`. It never
  calls `invoke_handler` or `setup`.
- In this task there are no tools and no commands. So `register` returns the
  builder unchanged, and there is no `invoke_handler` call yet. Its doc comment
  states the rules above and shows the shape M6 will fill in, as a code
  comment. Do not add a `generate_handler![]` with an empty list.
- **Enforcement.** A unit test in the shell crate,
  `builder_hooks_only_in_tool_registry`, sits in `src/tools/mod.rs` under
  `#[cfg(test)]`. It walks `apps/desktop/src-tauri/src/` (via
  `CARGO_MANIFEST_DIR`, read-only).
  - **It counts code, not comments.** In every file it scans, it first drops
    each line whose trimmed start is `//`, which covers `//`, `///`, and
    `//!`. Only the remaining lines are searched. The `//!` example of the
    shape M6 fills in, at the top of `tools/mod.rs`, is therefore never
    counted. Block comments (`/* */`) are not used for this and are not
    handled.
  - In `src/tools/mod.rs`, it scans only the text before `#[cfg(test)]`, so
    the test's own string literals are not counted.
  - It asserts:
    - no `.rs` file other than `src/tools/mod.rs` has a code line containing
      `.invoke_handler(` or `.setup(`;
    - `src/tools/mod.rs` has at most one code line with each.
  - The test's doc comment states both rules: comment lines are skipped in
    every file, and `mod.rs` is scanned only up to `#[cfg(test)]`. Then a
    later task knows that writing the method names in comments is fine.
  - Factor the filter into a small helper, for example
    `fn code_lines(text: &str) -> impl Iterator<Item = &str>`, used for every
    file.

**Frontend.** `src/tools/registry.ts` lists `{ id, label, view }` entries. A
tool's views live in `src/tools/<tool>/`, and its typed API in
`src/lib/<tool>.ts`.

Tools never import each other. Shared code goes in `fm-core`.

### Tauri configuration

- Target Windows.
- Minimum window size 800×560.
- Plugins: `tauri-plugin-dialog` and `tauri-plugin-opener`.
- Capabilities grant only what is used. For now that is core defaults plus the
  dialog and opener permissions. Do not add capabilities "for later".
- `bundle`: `"active": false`, the icons, and **no `targets` key** (portable
  exe, no installer).
- **Content Security Policy** (decided, replaces `"csp": null`). In
  `app.security`:

  ```json
  "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost; object-src 'none'; base-uri 'self'; form-action 'none'; frame-ancestors 'none'",
  "dangerousDisableAssetCspModification": ["style-src"]
  ```

  - Everything is served from the embedded frontend. There are no remote
    origins.
  - `connect-src ipc: http://ipc.localhost` is required for Tauri v2 IPC
    (`invoke` and events) on Windows.
  - `style-src 'unsafe-inline'` allows React `style` attributes, which the UI
    tasks may use (for example a progress bar width). Tauri would otherwise
    append hashes or nonces to `style-src`, and browsers ignore
    `'unsafe-inline'` once a nonce or hash is present. That is why
    `dangerousDisableAssetCspModification` is limited to `style-src`. Tauri
    still hashes `script-src`, and scripts stay `'self'` only.
  - `img-src data:` covers inline SVG and data-URI icons.
  - **Dev server.** Set `devCsp` to the same policy with
    `ws://localhost:1420 http://localhost:1420` added to `connect-src` (Vite
    HMR). If `npm run tauri:dev` still reports a CSP violation from the Vite
    React-refresh preamble, add `'unsafe-inline'` to `script-src` **in
    `devCsp` only**. Never loosen the production `csp`.
  - `tauri:dev` cannot run in the Linux agent container. So ship the stricter
    `devCsp` (without `'unsafe-inline'` in `script-src`), and say in your
    handoff that it is unverified. The Windows check in "Done condition"
    settles it.

### Frontend configuration

- `vite.config.ts` keeps `test.globals: false`. With globals off, Testing
  Library does not register its automatic cleanup. So
  `apps/desktop/src/test-setup.ts` must be:

  ```ts
  import "@testing-library/jest-dom/vitest";
  import { afterEach } from "vitest";
  import { cleanup } from "@testing-library/react";

  afterEach(() => {
    cleanup();
  });
  ```

- **Accessibility lint** (decided: it lands here, not in the first UI task).
  Add the dev dependency `eslint-plugin-jsx-a11y` (pin it in
  `apps/desktop/package.json` and update `package-lock.json`). In
  `eslint.config.js`, apply `jsxA11y.flatConfigs.recommended` to
  `**/*.{ts,tsx}`, alongside the existing typescript-eslint and react-hooks
  config. `npm run lint` keeps `--max-warnings 0`, so any a11y finding fails
  lint. `App.tsx` must pass it.

### Commands to make work and record in `CLAUDE.md`

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

Also record the PowerShell form of the last command (above).

**Frontend dist before cargo.** `tauri::generate_context!()` in `lib.rs` may
need `apps/desktop/dist` to exist at compile time. Check it once:

1. Move `apps/desktop/dist` aside.
2. Run `cargo check -p filemanager`.
3. Restore `dist`.

- If the check fails, add `npm run build -w apps/desktop` to `CLAUDE.md`
  Commands as "once before cargo commands that build the shell".
- Either way, CI builds the frontend before any workspace cargo step (below).

### CI (GitHub Actions)

`.github/workflows/ci.yml`. Both jobs run the whole workspace, so the shell
crate's tests, and with them the ts-rs currency test, run on both platforms.

**`linux` (ubuntu-latest).** Steps in this order:

1. Checkout, then the pinned Rust toolchain with `rustfmt` and `clippy`,
   `Swatinem/rust-cache`, and Node 22 with the npm cache.
2. Install the shell's system dependencies:

   ```sh
   sudo apt-get update
   sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev
   ```

   If the build then reports another missing system library, add the package
   that Tauri's Linux prerequisites name for it. Record the final list in
   `CLAUDE.md`.
3. `npm ci`, then `npm run build -w apps/desktop`.
4. `cargo fmt --all --check`.
5. `cargo clippy --workspace --all-targets --locked -- -D warnings`.
6. `cargo test --workspace --locked`.
7. `npm run typecheck`, `npm run lint`, `npm run test`.
8. The tauri-dependency check (below).

**`windows` (windows-latest, MSVC toolchain by default).** Steps in this order:

1. Checkout, toolchain, rust-cache, Node 22.
2. `npm ci`, then `npm run build -w apps/desktop`.
3. `cargo test --workspace --locked`.
4. `npm run tauri:build`.

Windows is the authoritative platform, so it runs the full workspace tests
too.

**The tauri-dependency check** passes only when cargo reports that `tauri` is
absent from the crate's graph. Any other failure, such as a resolver error,
fails the step. Checked output from this repo:

- `cargo tree -p fm-core -i tauri` exits 101 and prints
  ``error: package ID specification `tauri` did not match any packages``.
- A misspelled crate is **not** caught by `cargo tree` itself. With `-i`,
  cargo ignores a `-p` that matches nothing: `cargo tree -p fm-cor -i tauri`
  exits 0 and prints the whole inverse tree of `tauri`.
- `cargo pkgid -p fm-cor` exits 101, and `cargo pkgid -p fm-core` exits 0.

So each crate's existence is checked with `cargo pkgid` first:

```yaml
- name: fm-* crates must not depend on tauri
  shell: bash
  run: |
    set -uo pipefail
    for c in fm-core fm-tarpack; do
      if ! cargo pkgid --locked -p "$c" >/dev/null; then
        echo "::error::no workspace package named $c"; exit 1
      fi
      if out=$(cargo tree --locked -p "$c" -i tauri -e normal,build,dev 2>&1); then
        echo "::error::$c depends on tauri"; echo "$out"; exit 1
      fi
      if ! grep -q "did not match any packages" <<<"$out"; then
        echo "::error::unexpected cargo tree failure for $c"; echo "$out"; exit 1
      fi
    done
```

**Repository rules that bind this task** (from `CLAUDE.md`):

- No secrets.
- Configuration keys are documented in `.env.example`.
- The agent files (`.claude/agents`, `.codex/agents`) are not touched by this
  task.
- Tests never touch real user files. The only test that writes outside a temp
  dir is update mode, which is opt-in and writes only
  `apps/desktop/src/lib/generated/`.

## Files

Create or adjust everything in the layout above. Then edit `CLAUDE.md` so
that:

- **Status** no longer says the stack is unchosen.
- **Layout** lists the tree above exactly. It must include
  `.cargo/config.toml`, `.gitattributes`, `src-tauri/build.rs`,
  `src-tauri/icons/`, `src-tauri/src/generated_types.rs`, `src/App.test.tsx`,
  and `src/test-setup.ts`. It must **not** say `fm-tarpack` owns the export
  test. `src/lib.rs` is described as "builder, plugins, `tools::register`", not
  "invoke_handler".
- **Commands** lists the commands above, including the regenerate command in
  both shell forms, and notes that:
  - both CI jobs run `cargo test --workspace`;
  - building the shell on Linux needs the webkit2gtk dev packages (list them).
    Without them, cargo can be scoped to `-p fm-core -p fm-tarpack`, but then
    the generated-types test does not run;
  - `tauri:build` produces a portable exe (`target/release/filemanager.exe`),
    not an installer;
  - the native prerequisites are MSVC on Windows and a C compiler on Linux;
  - the frontend-dist note, if the check above showed it is needed.
- **Invariants** keeps every existing bullet and adds:
  - the boundary directory `apps/desktop/src/lib/`;
  - generated types in `lib/generated/`, never hand-edited. The only writer is
    `apps/desktop/src-tauri/src/generated_types.rs`, via the regenerate
    command. `#[ts(export)]` is not used. `ts-rs` is a workspace dependency
    that each crate uses directly;
  - the `fm-*` crates never depend on tauri;
  - only `tools::register` calls `invoke_handler` or `setup`, once each;
  - the webview CSP in `tauri.conf.json` is restrictive, and changing it is a
    reviewed decision, not a fix-up.
- A new **Adding a tool** section gives this six-step recipe, verbatim in
  substance:
  1. Add a crate `crates/fm-<tool>` with no tauri dependency. If it has
     boundary types, add `ts-rs = { workspace = true }` and derive
     `ts_rs::TS`.
  2. Add a module `src-tauri/src/tools/<tool>.rs`. It contributes only
     `#[tauri::command]` functions prefixed `<tool>_`, a managed-state
     constructor if needed, and optionally `setup(app)`. It never touches the
     `Builder`.
  3. In `tools/mod.rs::register`:
     - add the tool's state with `.manage(...)`;
     - add its commands to the **single** `tauri::generate_handler![...]`;
     - call its `setup` from the single `.setup(...)`, if it has one.

     Tauri keeps only the last `invoke_handler` or `setup`, so never add a
     second one.
  4. Append each boundary type's root to `EXPORTERS` in
     `src-tauri/src/generated_types.rs`, then run the regenerate command.
  5. Add a typed wrapper `src/lib/<tool>.ts`, and a view folder
     `src/tools/<tool>/` with an entry in `src/tools/registry.ts`.
  6. Keep persisted state under the tool's own namespace in `fm-core`'s store.

Keep `/apps/desktop/src-tauri/gen/` and `/apps/desktop/src-tauri/target/` in
`.gitignore`, as the first implementation added them.

## Acceptance criteria

- Every command above runs clean in this repository. This container has the
  webkit2gtk dev packages, so the full workspace builds and tests here.
  `tauri:dev` and `tauri:build` only need to succeed where a Tauri toolchain is
  present. The Windows CI job proves `tauri:build`.
- `fm-core` and `fm-tarpack` each build with one smoke test and have no tauri
  dependency. `fm-core` has no `ts-rs` dependency and no re-export.
- `ts-rs` appears once in `[workspace.dependencies]`, and crates use it as
  `{ workspace = true }`.
- The ts-rs export lives in `apps/desktop/src-tauri/src/generated_types.rs`,
  and `crates/fm-tarpack/tests/generated_types.rs` is gone.
- Running the regenerate command with the empty `EXPORTERS` list leaves
  `lib/generated/` holding only `.gitkeep`, and `git status` shows no change
  there. Check mode passes.
- Temporarily adding a file to `lib/generated/` makes
  `cargo test -p filemanager generated_types_are_current` fail, naming the file
  and the regenerate command. Update mode then removes it. Revert the
  experiment afterwards.
- `tools/mod.rs` documents the single-handler rule, `lib.rs` calls no
  `invoke_handler` or `setup`, and `builder_hooks_only_in_tool_registry`
  passes.
- The hook test ignores comments and counts real calls. Check it by hand, then
  revert:
  1. Change `register`'s body to `builder.setup(|_| Ok(()))`. The test passes,
     even with the `//!` example that also mentions `.setup(`.
  2. Chain a second `.setup(|_| Ok(()))`. The test fails and names
     `tools/mod.rs`.
  3. Put a `// .invoke_handler(` comment line in `lib.rs`. The test passes.
- **`TS_RS_EXPORT_DIR` backstop** (one-time check, then revert):
  1. Temporarily add `#[derive(ts_rs::TS)] #[ts(export)] struct StrayProbe;`
     inside the `#[cfg(test)]` module of `generated_types.rs`.
  2. Run `cargo test -p filemanager --lib export_bindings`.
  3. Check that `target/ts-rs-stray/StrayProbe.ts` exists, and that no
     `bindings/` directory appeared under `apps/desktop/src-tauri/` or
     anywhere else in the repo.
  4. Remove the probe and `target/ts-rs-stray/`.

  Report the result in your handoff.
- `tauri.conf.json` has no `bundle.targets`, and has the CSP,
  `dangerousDisableAssetCspModification`, and `devCsp` above.
- `test-setup.ts` registers `afterEach(cleanup)`.
- `npm run lint` runs `eslint-plugin-jsx-a11y`'s recommended rules. Adding
  `<img src="x" />` without `alt` to `App.tsx` makes lint fail. Revert the
  experiment afterwards.
- The CI workflow matches the CI section above, step for step. Run the
  dependency check's loop locally in bash:
  - with `fm-core fm-tarpack`, it passes;
  - with `fm-cor`, it fails with `::error::no workspace package named fm-cor`.
- `CLAUDE.md` matches the repository exactly.

## Tests proving completion

- `cargo test --workspace`:
  - `fm_core` `smoke` and `fm_tarpack` `smoke`;
  - in `filemanager`: `generated_types::generated_types_are_current`,
    `generated_types::sync_replaces_stale_files_and_keeps_gitkeep`, and
    `tools::tests::builder_hooks_only_in_tool_registry`.
- `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo fmt --all --check`.
- `npm run test`: `App.test.tsx`, which renders the placeholder.
- `npm run typecheck` and `npm run lint`.

## Done condition

The implementer run ends when every local check above passes and the diff is
handed to the reviewer. The implementer does not push.

**M1 counts as landed only when both of these hold:**

1. **Both CI jobs are green on the pushed branch.** The orchestrator pushes
   after review and confirms this.
2. **A one-time `npm run tauri:dev` on Windows**, done by the human or the
   orchestrator, not by the implementer. The window opens, the placeholder
   renders, and the devtools console shows no CSP violation.
   - If it fails with a CSP violation, the only permitted fix is loosening
     `devCsp`: add `'unsafe-inline'` to its `script-src`, or add the missing
     dev-server origin to its `connect-src`.
   - The production `csp` is never changed for this.
   - The fix goes back through an implementer run on this plan and the
     reviewer.

If either fails, the failure comes back as a new implementer run on this task
plan. Until M1 has landed, no feature task starts.

In your handoff, report:

- that `devCsp` is unverified, pending the Windows `tauri:dev` check;
- the `TS_RS_EXPORT_DIR` backstop check result;
- whether `generate_context!` needed `dist`;
- the final Linux package list.

## Out of scope

- No state store (M2), manifest (M3), archive (M4), or matching (M5).
- No tarpack commands (M6), and no `invoke_handler` call yet.
- No static CRT or artifact upload (M7).
- No visual design, tokens, or navigation UI; that is the `ui-implementer`'s
  UI task. The `App.tsx` placeholder is deliberately bare.

## Risks

- Tauri v2 and plugin versions must be pinned together.
- `tauri build --no-bundle` needs a recent Tauri CLI v2. Pin the CLI version
  in `apps/desktop/package.json`.
- Linux CI is slower now that it builds webkit-linked code. That is accepted,
  because it is the price of running the shell crate's tests on both jobs.
- `dangerousDisableAssetCspModification` only stops Tauri adding hashes to
  `style-src`. Scripts stay hash-protected. Do not widen it to `script-src`.
