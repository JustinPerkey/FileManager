# FileManager

A standalone application repository. It owns its own history, CI, and agent
chain.

## Status

Stack chosen: a Tauri v2 shell, Rust workspace crates for domain logic, a
React + TypeScript + Vite frontend, npm workspaces at the root. FileManager is
a Windows-only desktop app distributed as a portable `.exe` (no installer). It
hosts several independent tools: the Pipeline (`pipeline`, UI and plumbing
only: its four step hooks are stubs), the Tar Packager (`tarpack`) and the
Schedule Creator (`schedule`, UI and plumbing only: its parse and merge hooks
are stubs). The Database Manager (`dbmanager`) is in progress: crate
(`fm-dbmanager`) and, in the GPUI app, its tool core and read-side model
(views to come). Its one app-wide config is
`%ProgramData%\FileManager\dbmanager.toml` (`/etc/filemanager/dbmanager.toml`
elsewhere); the app only reads it, and a config that cannot be used fails
closed (everything read-only).

A GPUI app is being built beside the Tauri shell in `apps/filemanager` (package
`filemanager-gpui`); the Tauri exe still ships. The Pipeline's and the Schedule
Creator's model, backend trait, copy and fixtures are ported there, and so are
the Tar Packager's, manifest and build sides (their views are still
placeholders). It
moves the UI to GPUI with the gpui-component widgets; a later cutover milestone
deletes `apps/desktop` and renames the package and binary to `filemanager`.

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
- `devtools/task-plan-guard/` — Rust binary (workspace member) run from
  `.claude/settings.json` as a `PreToolUse` hook; guards only the `implementer`
  and `ui-implementer` subagents (by `agent_type`) and keeps them out of every
  plan except task plans. `TASK_PLAN_GUARD_TRACE=<file>` appends one line per call.
- `PRODUCT.md`, `DESIGN.md` — the `impeccable` skill's context: durable product
  truth, and the visual system (normative source: `styles/tokens.css`). Read
  by the UI agents; `DESIGN.md` changes when the tokens or shared components
  do.
- Application:

```
Cargo.toml                    workspace: members crates/*, devtools/*, apps/filemanager,
                              apps/desktop/src-tauri; [workspace.dependencies] pins
                              ts-rs, tempfile, notify-debouncer-mini, directories, gpui (gpui-pre
                              =0.3.12), gpui_platform (gpui-pre-platform =0.3.12, no
                              default features), gpui-component =0.10.0,
                              gpui-kit-assets =0.10.0, async-channel, log, windows (0.62;
                              the GPUI app's Windows dialogs), raw-window-handle;
                              [workspace.lints.clippy] turns on pedantic, and every
                              crate opts in with [lints] workspace = true
.cargo/config.toml            [env] TS_RS_EXPORT_DIR points stray #[ts(export)] into target/;
                              [target.x86_64-pc-windows-msvc] static CRT (+crt-static)
.gitattributes                apps/desktop/src/lib/generated/** text eol=lf
rust-toolchain.toml           stable channel (latest stable, not pinned)
rustfmt.toml
package.json                  root scripts, npm workspaces: apps/desktop
devtools/
  task-plan-guard/            the PreToolUse hook binary (serde_json only); `decide` in
                              src/lib.rs, table-driven tests in tests/cases.rs
crates/
  fm-core/                    shared, tool-agnostic library. No tauri dependency.
                              `watch.rs` is FileWatcher (debounced watch of one file's
                              parent directory, notify-debouncer-mini)
  fm-dbmanager/               Database Manager domain library. Depends on fm-core only.
                              No tauri dependency. `db.rs` opens a SQLite file;
                              `state.rs` is the recent list, persisted as dbmanager/state;
                              `config/load.rs` reads the one config file (LoadedConfig);
                              `src/session/` is Session: reads (list, object, m2m_options),
                              schema/config change detection, and save (`save.rs`: checks,
                              reference resolution, one BEGIN IMMEDIATE transaction;
                              `links.rs`: many-to-many link adds, updates and removes in
                              that same transaction, strict conflicts;
                              `backup.rs`: the one backup before a session's first write;
                              `delete.rs`: the delete impact preview and the confirmed delete);
                              `session/sql.rs` is the only module that assembles SQL text;
                              `src/tool/` is the tool core (Core: the app-wide config, the
                              recent databases, open/drop/reload/close, DbmanagerSnapshot,
                              DbmanagerError and its 22 kinds); plain Rust, not exported
                              through ts-rs
    src/value.rs              DbValue (boundary value; integers, reals, durations as strings)
    src/rules/                FieldRule/FieldKind/FieldIssue types, pure validate, and
                              duration.rs (the duration grammar, parse, format; config
                              reuses the grammar); kinds.rs
                              (infer_kind, `type` override rules, default literals)
    src/model/                resolve(&Schema, &ConfigReport) -> Model: the config laid over
                              the schema (modes, kinds, rules, labels, links, reverse and
                              many-to-many relations, diagnostics); views.rs holds the
                              exported TableView and friends
    src/config/               the TOML config overlay: parse (text -> Config with
                              line/col diagnostics), model, validation. Checks
                              what the text alone shows; schema-dependent rules come later
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
    src/session/              Core (session logic with no UI types), DropJob, BuildJob,
                              the boundary types exported through ts-rs, and the
                              progress ProgressCoalescer
  fm-schedule/                Schedule Creator domain library. Depends on fm-core only.
                              No tauri dependency.
                              parse (text -> Schedule) and merge (Schedule + XML -> XML)
                              are the hooks, run only on apply; both are stubs
                              returning NotImplemented. `src/session/` is the
                              session logic (Core, Hooks, backup, atomic write),
                              its types and tests, with no UI types
  fm-pipeline/                Pipeline domain library. No tauri dependency.
                              subdirectories() lists the units of work; one hook
                              per step (verify, batch, schedule, tarpack), all
                              stubs returning StepError::NotImplemented.
                              `src/session/` is the session logic (Core, Dir, Job,
                              Finished), its types and tests, with no UI types
examples/tarpack/             example manifests, valid, used by tests through include_str!
examples/dbmanager/           schema.sql (example database), example.toml (its config),
                              starter.toml (the inert commented starter file), and
                              validation-cases.json (the shared validate vectors: Rust
                              test now, vitest mirror later); used by tests through
                              include_str!
docs/tarpack-manifest.md      the manifest, archive-layout, and extraction reference
docs/tarpack-e2e.md           manual end-to-end release checklist (Windows exe to Linux target)
docs/dbmanager-config.md      the Database Manager config reference (the format's only home)
docs/schedule.md              the Schedule Creator's flow and where its hooks go
docs/pipeline.md              the Pipeline's flow and where its step hooks go
docs/manual/user-manual.md    end-user manual; images/ are its screenshots, regenerated
                              by screenshots/shoot.js (real frontend, mocked backend)
apps/filemanager/             GPUI app: package and binary `filemanager-gpui`, lib `fm_app`
                              (the lib name survives the later rename). Linux builds a
                              headless backend and runs tests only; the window needs Windows.
  build.rs                    embeds assets/icon.ico as resource id 1 (Windows)
  assets/                     icon.ico, icons/*.svg (no font files: fonts are system fonts)
  src/main.rs                 parses `--self-check`, calls fm_app::run
  src/lib.rs                  run(): gpui_component::init, mono font, dev mode, installs the
                              platform (GpuiPlatform) and the path-loss logger, main window
  src/assets.rs, fonts.rs     embedded icons (icons! list); system fonts (Segoe UI, mono
                              Cascadia Mono else Consolas)
  src/platform/               Platform trait (pick(OpenSpec), pick_save(SaveSpec), open and
                              reveal (each a Task<Result<(), OpenError>>), copy_text; global PlatformHandle), gpui.rs (real),
                              gate.rs (DialogGate, run_dialog: one dialog at a time),
                              win_dialog.rs (Windows only: the common item dialog over the
                              `windows` crate, filters, default extension, exact OsString
                              paths; manual check in docs/gpui-dialogs-check.md,
                              examples/dialog_check.rs),
                              win_shell.rs (Windows only: open_with_system and reveal
                              over ShellExecuteW / SHOpenFolderAndSelectItems on a COM
                              thread, returning Task<Result<(), OpenError>>; with
                              win_dialog.rs the only `unsafe` in the app),
                              fake.rs (scripted, logs specs, for tests and screenshots),
                              path_loss.rs (log::Log that reports dropped names GPUI skips)
  src/jobs.rs                 Worker (backend off the UI thread) and spawn_thread (long
                              jobs with progress); models never block the UI thread
  src/app/                    shell.rs (Shell root view: current tool, drop routing,
                              notice, close request), drop.rs (DropTarget), guard.rs
                              (CloseRequest, unsaved work from the tools),
                              dirs.rs (user_app_dirs, system_config_path,
                              user_documents_dir: the only callers of the real
                              folders; the first two panic in test and `shoot`
                              builds, the third returns a fake path; the main
                              `filemanager-gpui` binary built with `--features
                              shoot` panics at startup by design, only
                              `filemanager-shoot` runs with that feature),
                              dev_mode.rs (Ctrl+Alt+Shift+D global, keystroke interceptor), text.rs,
                              view/ (the chrome slot: ui-implementer's, placeholder.rs
                              is the plain stand-in)
  src/app/path.rs             clean_path, shared by the tools' models
  src/app/probe.rs            `.tag("name")` on an element: records its bounds (test and
                              `shoot` builds only) so the harness can click or clip to it;
                              only tags drawn in the latest frame resolve (Shell calls
                              `begin_frame` once per draw); `.tag` goes last in a chain
  src/shoot/                  screenshot harness, feature `shoot` only: Scenario/Step API,
                              steps.rs (player), cli.rs (arg parsing, child runner with
                              a 60 s timeout), scenarios/ (ui-implementer's; one per
                              manual image). src/bin/filemanager-shoot.rs is its binary
                              (required-features = shoot); never in the release exe
  src/tools/                  registry (ToolId, ToolSlot, ToolHooks, build) and one
                              module per tool (Pipeline, Schedule and Tar Packager have their real
                              models, backends, text and fixtures), each with a view/
                              placeholder; tools never import each other
  src/tools/dbmanager/        backend.rs (DbmanagerBackend over fm_dbmanager's tool Core;
                              RealDbmanager), model.rs (DbmanagerModel, read side: places and
                              history, lists, object, referenced-by, many-to-many), text.rs,
                              fixtures.rs (FixtureEnv: a real Core over a temp-dir database;
                              test and `shoot` builds), tests.rs; view/ is a placeholder
  src/tools/schedule/         same shape as pipeline/ (ScheduleBackend over fm_schedule's
                              Core; RealSchedule, ScheduleModel, text.rs, fixtures.rs,
                              tests.rs); view/ is a placeholder
  src/tools/pipeline/         backend.rs (PipelineBackend over fm_pipeline's Core;
                              RealPipeline), model.rs (PipelineModel: all behaviour),
                              text.rs (the copy), fixtures.rs (FixturePipeline, test and
                              `shoot` builds), tests.rs; view/ is a placeholder
docs/gpui-test-ledger.md      every frontend test case and where it goes in the GPUI app
apps/desktop/
  package.json, vite.config.ts, tsconfig.json, .oxlintrc.json, index.html
  src-tauri/                  package and binary name: filemanager
    Cargo.toml
    build.rs                  tauri_build::build()
    tauri.conf.json, capabilities/default.json
    icons/
    src/main.rs               calls filemanager_lib::run()
    src/lib.rs                builder, plugins, tools::register
    src/tools/mod.rs          tool registry: the only place that calls
                              invoke_handler / setup
    src/tools/tarpack/        mod.rs only (the tarpack_* commands, managed state and
                              manifest watcher arming over fm_tarpack::session)
    src/tools/schedule/       mod.rs only (the schedule_* commands over
                              fm_schedule::session)
    src/tools/pipeline/       mod.rs only (the pipeline_* commands over
                              fm_pipeline::session; a step runs on a blocking
                              worker)
    src/generated_types.rs    #[cfg(test)] ts-rs export and currency test
  src/
    main.tsx, App.tsx
    App.test.tsx
    test-setup.ts             jest-dom matchers + afterEach(cleanup)
    lib/tauri.ts              thin wrappers over @tauri-apps/api (invoke, listen)
    lib/tarpack.ts            typed client: one function per tarpack_* command,
                              onManifestChanged, onBuildProgress
    lib/schedule.ts           typed client: one function per schedule_* command
    lib/pipeline.ts           typed client: one function per pipeline_* command
    lib/generated/            TS types generated from Rust (ts-rs)
    tools/registry.ts         export const tools: ToolEntry[]
.github/workflows/ci.yml      linux job: fmt, clippy, tests, the five-crate dependency
                              check, the gpui test-support check. windows job
                              (windows-2025): clippy, tests, both release builds,
                              dumpbin on both exes, the GPUI self-check; uploads
                              FileManager-<version>-x64.exe and
                              FileManager-gpui-<version>-x64.exe
.env.example                  configuration keys (none yet)
```

## Commands

```sh
npm install                                        # once, at repo root
npm run tauri:dev                                  # run the app
npm run tauri:build                                # portable Windows exe (tauri build --no-bundle)
cargo run -p filemanager-gpui                      # run the GPUI app (Windows; `-- --self-check` quits after one frame)
cargo run -p filemanager-gpui --features shoot --bin filemanager-shoot -- --out shots   # screenshots (Windows, 100% scale)
cargo clippy -p filemanager-gpui --features shoot --all-targets -- -D warnings          # the harness is not in the workspace run
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings   # includes clippy::pedantic (workspace lints)
cargo test --workspace
npm run typecheck && npm run lint && npm run test  # frontend (vitest; lint: oxlint, jsx-a11y)
UPDATE_GENERATED=1 cargo test -p filemanager --lib generated_types_are_current   # regenerate lib/generated/
```

PowerShell form of the regenerate command:

```powershell
$env:UPDATE_GENERATED=1; cargo test -p filemanager --lib generated_types_are_current; Remove-Item Env:UPDATE_GENERATED
```

- `.cargo/config.toml` links the C runtime statically for
  `x86_64-pc-windows-msvc` (`+crt-static`), so the portable exe needs no VC++
  redistributable; `cc` then builds zstd/liblzma with `/MT`.
  `build.windows.staticVCRuntime` is `false` in `tauri.conf.json` on purpose:
  tauri-build's default (`true`) re-links the UCRT dynamically. A `RUSTFLAGS`
  environment variable overrides the config's rustflags (dropping
  `+crt-static`), and `STATIC_VCRUNTIME` overrides `staticVCRuntime`, so set
  neither. The Windows CI job runs `dumpbin /dependents` on both exes (`filemanager.exe`,
  `filemanager-gpui.exe`) and fails if
  it lists `vcruntime*`, `msvcp*`, `api-ms-win-crt-*`, `zstd*`, `liblzma*`,
  `sqlite3*`, or `WebView2Loader.dll`, then uploads
  `FileManager-<version>-x64.exe` and `FileManager-gpui-<version>-x64.exe`.
- Both CI jobs run `cargo test --workspace`, so the generated-types test runs
  on both.
- Building the shell on Linux needs the webkit2gtk dev packages:
  `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev`,
  `libjavascriptcoregtk-4.1-dev`. Without them, cargo can be scoped to
  `-p fm-core -p fm-tarpack -p fm-dbmanager -p filemanager-gpui -p task-plan-guard`
  (the GPUI app builds headless on Linux, without the webkit packages), but then the
  generated-types test does not run.
- `tauri:build` produces a portable exe (`target/release/filemanager.exe`), not
  an installer.
- Native prerequisites: `fm-tarpack` depends on `zstd` and `liblzma`
  (static), which compile bundled C sources. Windows needs the MSVC toolchain
  (Visual Studio Build Tools, "Desktop development with C++"), which Tauri
  already requires. Linux needs a C compiler (`cc`/`gcc`).
- `fm-dbmanager` depends on `rusqlite` with `bundled`, which compiles SQLite's
  C source with the same toolchain and static CRT as zstd/liblzma; the dumpbin
  check forbids `sqlite3*.dll`.
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
- The release exe never contains GPUI's `test-support` or the screenshot
  harness: the release build is `cargo build --release -p filemanager-gpui`
  with default features, and CI asserts `target\release\filemanager-shoot.exe`
  does not exist.
- Rust code is always `cargo fmt` clean and passes clippy with the `pedantic`
  group (set in `[workspace.lints.clippy]`; both CI jobs run clippy with
  `-D warnings`, so a pedantic finding fails the build, and the windows job
  covers `#[cfg(windows)]` code). A new crate adds `[lints] workspace =
  true`. Fix the finding rather than silencing it; when silencing is right, use
  a scoped `#[expect(clippy::<lint>, reason = "...")]`, never a crate-wide
  `allow`.
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

1. Add a crate `crates/fm-<tool>` with no tauri dependency, and
   `[lints] workspace = true` in its `Cargo.toml`. If it has boundary
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
