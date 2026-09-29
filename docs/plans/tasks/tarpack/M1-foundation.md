# M1 — Foundation: workspace, Tauri shell, tool registry, CI

Status: awaiting approval
Project: tarpack   Depends on: none

## Goal

Establish the stack, the repository layout, the core/UI boundary, the
verification commands, and CI, and record all of it in `CLAUDE.md`, so that
every later task builds on a stated foundation.

## Context

**The product.** FileManager is a Rust desktop application that runs only on
Windows. It will hold several independent tools ("subcomponents"). The first
tool is the Tar Packager (`tarpack`), which builds a Linux `.tar` from Windows
files. This task builds no tool logic. It builds the frame every tool plugs
into.

**Stack.**

- A Tauri v2 shell.
- Rust workspace crates for all domain logic.
- A React + TypeScript + Vite frontend.
- npm workspaces at the root.

**Layout to create.**

```
Cargo.toml                    workspace: members crates/*, apps/desktop/src-tauri
rust-toolchain.toml           stable, pinned to a specific version
rustfmt.toml
package.json                  root scripts (below), npm workspaces: apps/desktop
crates/
  fm-core/                    shared, tool-agnostic library. No tauri dependency.
  fm-tarpack/                 Tar Packager domain library. Depends on fm-core only.
                              No tauri dependency.
apps/desktop/
  package.json, vite.config.ts, tsconfig.json, eslint config, index.html
  src-tauri/
    Cargo.toml                package and binary name: filemanager
    tauri.conf.json
    capabilities/default.json
    src/main.rs, src/lib.rs   builder, plugins, invoke_handler
    src/tools/mod.rs          tool registry (empty for now; see below)
  src/
    main.tsx, App.tsx         minimal placeholder that renders the app name
    lib/tauri.ts              thin wrappers over @tauri-apps/api (invoke, listen)
    lib/generated/            TS types generated from Rust (ts-rs); starts empty
    tools/registry.ts         export const tools: ToolEntry[] = []
.github/workflows/ci.yml
.env.example                  a comment stating no configuration keys exist yet
```

**The core/UI boundary** falls at `apps/desktop/src/lib/`.

- The backend `implementer` owns everything in Rust and everything under
  `lib/`.
- The `ui-implementer` owns everything else under `apps/desktop/src/`.
- Types that cross the boundary are defined once in Rust with
  `#[derive(ts_rs::TS)]` and exported into `apps/desktop/src/lib/generated/`.
  No hand-written TS duplicates.

**Tool registry pattern**, which later tools follow:

- Rust: `src-tauri/src/tools/mod.rs` exposes one function that registers every
  tool's commands and managed state on the Tauri builder. Each tool is a module
  `tools/<tool>.rs`, and all its command names are prefixed `<tool>_`.
- Frontend: `src/tools/registry.ts` lists `{ id, label, view }` entries. A
  tool's views live in `src/tools/<tool>/`, and its typed API in
  `src/lib/<tool>.ts`.
- Tools never import each other. Shared code goes in `fm-core`.

**Tauri configuration.**

- Target Windows.
- Minimum window size 800×560.
- Plugins: `tauri-plugin-dialog` and `tauri-plugin-opener`.
- Capabilities grant only what is used. For now that is core defaults plus the
  dialog and opener permissions.

**Commands to make work and record in `CLAUDE.md`.**

```sh
npm install                                        # once, at repo root
npm run tauri:dev                                  # run the app
npm run tauri:build                                # Windows bundle
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run typecheck && npm run lint && npm run test  # frontend (vitest)
```

**CI** (GitHub Actions) has two jobs:

- **ubuntu-latest:**
  - `cargo fmt --check`;
  - clippy and tests scoped to the `fm-*` crates
    (`-p fm-core -p fm-tarpack`). The Tauri shell needs webkit2gtk to build on
    Linux, so it is excluded here;
  - frontend typecheck, lint, and test;
  - `cargo tree -p fm-core -i tauri` and `cargo tree -p fm-tarpack -i tauri`
    must find nothing.
- **windows-latest:** `cargo test -p fm-core -p fm-tarpack` and
  `npm run tauri:build`.

**Repository rules that bind this task** (from `CLAUDE.md`):

- No secrets.
- Configuration keys are documented in `.env.example`.
- The agent files are not touched by this task.

## Files

Create everything in the layout above. Edit `CLAUDE.md` so that:

- **Status** no longer says the stack is unchosen.
- **Layout** lists the tree above.
- **Commands** lists the commands above. Note that the shell builds only on
  Windows CI, or on Linux with webkit2gtk installed.
- **Invariants** keeps every existing bullet and adds:
  - the boundary directory `apps/desktop/src/lib/`;
  - generated types in `lib/generated/`, never hand-edited;
  - the `fm-*` crates never depend on tauri.
- A new **Adding a tool** section gives the five-step recipe:
  1. Add a crate `crates/fm-<tool>` with no tauri dependency.
  2. Add a module `src-tauri/src/tools/<tool>.rs` whose commands are prefixed
     `<tool>_`, registered through `tools/mod.rs`.
  3. Add a typed wrapper `src/lib/<tool>.ts`.
  4. Add a view folder `src/tools/<tool>/` and an entry in
     `src/tools/registry.ts`.
  5. Keep persisted state under the tool's own namespace in `fm-core`'s store.

Also append `/apps/desktop/src-tauri/gen/` and `/apps/desktop/src-tauri/target/`
to `.gitignore` if Tauri generates them.

## Acceptance criteria

- Every command above runs clean in this repository. `tauri:dev` and
  `tauri:build` only need to succeed where a Tauri toolchain is present; the
  Windows CI job proves `tauri:build`.
- `fm-core` and `fm-tarpack` each build with one smoke test and have no tauri
  dependency.
- The `ts-rs` export is wired: a test exports all `#[derive(TS)]` types and
  fails if the files in `lib/generated/` differ from a fresh export. With no
  types yet, it must pass on an empty set and be ready for M3 onward.
- `CLAUDE.md` matches the repository exactly.

## Tests proving completion

- `cargo test --workspace`: `fm_core::smoke`, `fm_tarpack::smoke`, and
  `generated_types_are_current` (in the crate that owns the export).
- `npm run test`: `App.test.tsx`, which renders the placeholder.
- Both CI jobs green on the pushed branch.

## Out of scope

- No state store (M2), manifest (M3), archive (M4), or matching (M5).
- No tarpack commands (M6).
- No visual design, tokens, or navigation UI; that is the `ui-implementer`'s
  UI task. The `App.tsx` placeholder is deliberately bare.

## Risks

- Tauri v2 and plugin versions must be pinned together.
- Do not add capabilities "for later".
