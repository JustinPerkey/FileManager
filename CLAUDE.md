# FileManager

A standalone application repository. It owns its own history, CI, and agent
chain.

## Status

Stack chosen: a Tauri v2 shell, Rust workspace crates for domain logic, a
React + TypeScript + Vite frontend, npm workspaces at the root. FileManager is
a Windows-only desktop app distributed as a portable `.exe` (no installer). It
hosts several independent tools; the first is the Tar Packager (`tarpack`).

## Layout

- `.codex/agents`, `.claude/agents` — planner / ui-designer / implementer /
  ui-implementer / reviewer roles, defined once per runtime and kept in sync.
- `docs/plans/project/` — project plans: architecture, decisions, task index,
  open questions. Never read by implementers.
- `docs/plans/tasks/<slug>/` — task plans: one self-contained brief per
  implementer or ui-implementer run (`M<n>-*.md` backend, `U<n>-*.md` UI).
  `docs/plans/README.md` defines both kinds.
- `.claude/settings.json` — enables the `impeccable` plugin (marketplace
  `pbakaus/impeccable`): one skill, `impeccable:impeccable`, whose
  sub-commands (`shape`, `critique`, `audit`, `polish`, …) the UI roles use.
- `.claude/hooks/task-plan-only.mjs` — keeps the implementer subagents out of
  every plan except task plans.
- `PRODUCT.md`, `DESIGN.md` — the `impeccable` skill's context: durable product
  truth, and the visual system (normative source: `styles/tokens.css`). Read
  by the UI agents; `DESIGN.md` changes when the tokens or shared components
  do.
- Application:

```
Cargo.toml                    workspace: members crates/*, apps/desktop/src-tauri;
                              [workspace.dependencies] pins ts-rs (and tempfile)
.cargo/config.toml            [env] TS_RS_EXPORT_DIR points stray #[ts(export)] into target/
.gitattributes                apps/desktop/src/lib/generated/** text eol=lf
rust-toolchain.toml           stable channel (latest stable, not pinned)
rustfmt.toml
package.json                  root scripts, npm workspaces: apps/desktop
crates/
  fm-core/                    shared, tool-agnostic library. No tauri dependency.
  fm-tarpack/                 Tar Packager domain library. Depends on fm-core only.
                              No tauri dependency.
    src/manifest/             parse, validate, model, views
    src/format.rs             ArchiveFormat, compression constants
    src/sources/              Assignments (entry id -> Windows source file), drop
                              matching (match_dropped, apply), and remembered
                              state (RememberedState, persisted as tarpack/state
                              in fm-core's Store)
    src/archive/              plan, header, eol, encode, write, verify:
                              atomic archive writer and post-write verification
examples/tarpack/             example manifests, valid, used by tests through include_str!
docs/tarpack-manifest.md      the manifest, archive-layout, and extraction reference
apps/desktop/
  package.json, vite.config.ts, tsconfig.json, eslint.config.js, index.html
  src-tauri/                  package and binary name: filemanager
    Cargo.toml                (incl. notify-debouncer-mini: manifest file watcher)
    build.rs                  tauri_build::build()
    tauri.conf.json, capabilities/default.json
    icons/
    src/main.rs               calls filemanager_lib::run()
    src/lib.rs                builder, plugins, tools::register
    src/tools/mod.rs          tool registry: the only place that calls
                              invoke_handler / setup
    src/tools/tarpack/        mod.rs (the tarpack_* commands and managed state),
                              core.rs (session logic as plain functions, no
                              Tauri types), progress.rs (ProgressCoalescer),
                              watch.rs (manifest watcher, notify-debouncer-mini),
                              types.rs (boundary types exported through ts-rs),
                              tests.rs
    src/generated_types.rs    #[cfg(test)] ts-rs export and currency test
  src/
    main.tsx, App.tsx
    App.test.tsx
    test-setup.ts             jest-dom matchers + afterEach(cleanup)
    lib/tauri.ts              thin wrappers over @tauri-apps/api (invoke, listen)
    lib/tarpack.ts            typed client: one function per tarpack_* command,
                              onManifestChanged, onBuildProgress
    lib/generated/            TS types generated from Rust (ts-rs)
    tools/registry.ts         export const tools: ToolEntry[]
.github/workflows/ci.yml      linux and windows jobs, both run cargo test --workspace
.env.example                  configuration keys (none yet)
```

## Commands

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

PowerShell form of the regenerate command:

```powershell
$env:UPDATE_GENERATED=1; cargo test -p filemanager --lib generated_types_are_current; Remove-Item Env:UPDATE_GENERATED
```

- `.cargo/config.toml` links the C runtime statically for
  `x86_64-pc-windows-msvc` (`+crt-static`), so the portable exe needs no VC++
  redistributable; `cc` then builds zstd/liblzma with `/MT`. The Windows CI job
  runs `dumpbin /dependents` on the exe and fails if it lists `vcruntime*`,
  `msvcp*`, `api-ms-win-crt-*`, `zstd*`, or `liblzma*` DLLs, then uploads
  `FileManager-<version>-x64.exe`. `docs/tarpack-e2e.md` is the manual
  end-to-end checklist.
- Both CI jobs run `cargo test --workspace`, so the generated-types test runs
  on both.
- Building the shell on Linux needs the webkit2gtk dev packages:
  `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev`,
  `libjavascriptcoregtk-4.1-dev`. Without them, cargo can be scoped to
  `-p fm-core -p fm-tarpack`, but then the generated-types test does not run.
- `tauri:build` produces a portable exe (`target/release/filemanager.exe`), not
  an installer.
- Native prerequisites: `fm-tarpack` depends on `zstd` and `liblzma`
  (static), which compile bundled C sources. Windows needs the MSVC toolchain
  (Visual Studio Build Tools, "Desktop development with C++"), which Tauri
  already requires. Linux needs a C compiler (`cc`/`gcc`).
- `tauri::generate_context!()` does not need `apps/desktop/dist` to exist for
  `cargo check`; CI builds the frontend first anyway.

## Invariants

- A type that crosses the core/UI boundary is defined once and mirrored
  explicitly on the other side. Change both or neither.
- Destructive filesystem operations — delete, overwrite, move onto an existing
  path — are never silent. They are confirmed or recoverable, and a failure part
  way through a batch is reported, not swallowed.
- Tests never touch real user files. Every test that exercises the filesystem
  runs inside a temporary directory it creates and removes.
- Paths are handled as the platform's path type, not as display strings. Code
  that renders a path does not assume it is valid UTF-8.
- No secrets in the repository. Configuration comes from the environment, with a
  checked-in `.env.example` documenting every key.
- Accessibility is a requirement, not a polish item. Every interactive element
  is keyboard reachable and labelled.
- Each `.claude` agent and its `.codex` counterpart are the same role written
  for two runtimes. Change one, change the other.
- No code change is complete until the `reviewer` has seen it. An implementer
  run ends by handing its diff to the reviewer.
- The core/UI boundary is `apps/desktop/src/lib/`. The backend `implementer`
  owns all Rust and everything under `lib/`; the `ui-implementer` owns the rest
  of `apps/desktop/src/`.
- Files in `lib/generated/` are generated by ts-rs from Rust types and never
  hand-edited. The only writer is `apps/desktop/src-tauri/src/generated_types.rs`,
  via the regenerate command. `#[ts(export)]` is not used. `ts-rs` is a
  workspace dependency that each crate uses directly.
- The `fm-*` crates never depend on tauri. Tools never import each other;
  shared code goes in `fm-core`.
- Only `tools::register` calls `invoke_handler` or `setup`, once each (a test
  enforces it). Tauri keeps only the last call of either.
- The webview CSP in `tauri.conf.json` is restrictive. Changing it is a
  reviewed decision, not a fix-up.

## Adding a tool

1. Add a crate `crates/fm-<tool>` with no tauri dependency. If it has boundary
   types, add `ts-rs = { workspace = true }` and derive `ts_rs::TS`.
2. Add a module `src-tauri/src/tools/<tool>.rs`. It contributes only
   `#[tauri::command]` functions prefixed `<tool>_`, a managed-state
   constructor if needed, and optionally `setup(app)`. It never touches the
   `Builder`.
3. In `tools/mod.rs::register`:
   - add the tool's state with `.manage(...)`;
   - add its commands to the **single** `tauri::generate_handler![...]`;
   - call its `setup` from the single `.setup(...)`, if it has one.

   Tauri keeps only the last `invoke_handler` or `setup`, so never add a second
   one.
4. Append each boundary type's root to `EXPORTERS` in
   `src-tauri/src/generated_types.rs`, then run the regenerate command.
5. Add a typed wrapper `src/lib/<tool>.ts`, and a view folder
   `src/tools/<tool>/` with an entry in `src/tools/registry.ts`.
6. Keep persisted state under the tool's own namespace in `fm-core`'s store.

## Working style

Substantial changes go through the chain: planner → (ui-designer, when the
change has a visual surface) → approval → implementer and ui-implementer, one
task plan per run → reviewer.

When spawning an `implementer` or `ui-implementer`, give it the path of exactly
one task plan in `docs/plans/tasks/`, and nothing from the project plan: no
summary, no pasted excerpt. The task plan is self-contained. If an implementer
reports a gap in it, send the gap back to the planner or ui-designer to fix the
task plan; do not patch it in the prompt.

UI work uses the `impeccable` skill (`/impeccable <command>`): `ui-designer`
shapes and critiques, `ui-implementer` builds and polishes, both audit before
declaring done. The
backend `implementer` does not make visual decisions; `ui-implementer` does not
touch backend source. The two meet at the UI-facing half of the core contract,
which the `implementer` owns.
