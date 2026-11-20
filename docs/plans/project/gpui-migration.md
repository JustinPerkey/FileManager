# Project plan: migrating the UI from Tauri + React to GPUI + gpui-kit

Status: **awaiting approval.** Written by the planner on 2026-11-19, after the
human's decisions on [`gpui-ce-assessment.md`](gpui-ce-assessment.md) (§1.1).
Revised 2026-11-20: the human answered Q4, Q5, Q6 and Q12 (D6–D9 in §1.1,
§14.1); the spike task plans were updated to match.

Only the spike (M1) has full task plans. Every milestone after it is listed in
§9 with its goal, dependencies and acceptance in brief. Their task plans are
written after the human accepts the spike report (§8), so that they are built
on measured facts, not on the unknowns listed in §2.

The visual half (theme port, shared widgets, per-tool views, screenshot
scenarios) goes to the `ui-designer` (§13), who writes
`docs/plans/project/gpui-migration-ui.md` and the `U<n>` task plans under
`docs/plans/tasks/gpui-migration/`. That plan does not exist yet.

Task plans: [`../tasks/gpui-migration/`](../tasks/gpui-migration/).

## 1. Request

Rewrite FileManager's UI in **upstream Zed GPUI** with the longbridge
**gpui-kit** component library (crate `gpui-component`). Remove Tauri, React,
the TypeScript frontend, ts-rs and npm/Node from the application entirely.

### 1.1 The human's decisions (2026-11-19)

| # | Decision |
|---|---|
| D1 | **Full rewrite.** Every tool (Pipeline, Tar Packager, Schedule Creator, and the Database Manager's coming UI) moves to GPUI. Tauri, React, the TS frontend and npm/Node are removed. |
| D2 | **Upstream Zed GPUI + gpui-kit**, not GPUI CE (the assessment's option D). |
| D3 | **The UI looks similar to today:** same layout, tokens (colours, spacing, type scale, radii), components and states as `DESIGN.md` and `apps/desktop/src/styles/tokens.css` describe. Light and dark, following the system, as today. |
| D4 | **The manual's screenshots stay auto-generated** from the real UI with a mocked or fixture backend, without Node. |
| D5 | **Accessibility is no longer a hard requirement.** Keep keyboard operation and labels where gpui-kit gives them cheaply; screen-reader support is not a gate. The `CLAUDE.md` invariant change is a decision the human approves explicitly (§10.3, Q1). |
| D6 | **Spike location (Q4):** new branch `spike/gpui-migration`, standalone `spike/gpui/` workspace with its own workflow, never merged. Pushing that branch is approved. |
| D7 | **Similarity (Q5):** same layout and token colours. Sampled colours within ±1 per channel, region geometry within about 2 px of today's images; text rendering may differ. |
| D8 | **Exe size and build time (Q6):** no limit. Measured and reported only. |
| D9 | **Minimum Windows (Q12):** **Windows 11 only.** Windows 10 is not supported. |

### 1.2 What does not change

- The domain crates (`fm-core`, `fm-tarpack`, `fm-schedule`, `fm-pipeline`,
  `fm-dbmanager`) and their behaviour. None depends on a UI today.
- The product: four tools, the same flows, copy and confirmations
  (`PRODUCT.md`), a portable Windows `.exe` with no installer, statically
  linked C runtime, the `dumpbin` dependency check.
- The invariants on destructive operations, temp-dir tests, OS path types,
  `cargo fmt` + clippy `pedantic`, no secrets, agent-role sync, and review.
- The Database Manager's domain milestones M7–M9 (§11).

## 2. Starting facts and unknowns

From the assessment (fetched 2026-11-19), restated so this plan stands alone:

- `gpui-component` 0.10.0 (2026-11-16), Apache-2.0, roughly weekly releases.
  It pins GPUI as the crates.io snapshot `gpui-pre` at an exact version
  (`=0.3.12`), because "any snapshot may change GPUI's API". Each gpui-kit
  upgrade is a GPUI upgrade. The crates.io `gpui` (0.2.2, 2025-10-22) is a
  year old and is **not** what gpui-kit builds on.
- GPUI is pre-1.0, "often breaking", with no promise to outside users.
- Zed has shipped stable on Windows since 2025-10-15 (DirectX 11,
  DirectWrite), so the Windows backend carries a production editor.
- gpui-kit advertises 60–75+ components including virtual lists, virtual
  data tables, forms, dialogs, menus and popovers, and ships in a commercial
  app (Longbridge Pro).

**Unconfirmed**, and each one is a spike item (§8): static CRT with no
forbidden DLL; native open/save dialogs with filters; Explorer drop
delivering paths; cancellable window close; opening a file in the system
editor; a tri-state checkbox; theming gpui-kit from our tokens closely
enough; rendering a window to PNG unattended; `#[gpui::test]` on Windows CI;
GPUI on a GPU-less CI runner (WARP); exe size and build time; the minimum
Windows version; whether clippy `pedantic` is workable on GPUI code; GPUI on
Linux CI.

## 3. Architecture after the migration *(proposed; confirmed after the spike)*

### 3.1 Layout

```
Cargo.toml                      workspace: members crates/*, apps/filemanager
                                [workspace.dependencies] pins gpui (gpui-pre, exact),
                                gpui-component (exact), tempfile; ts-rs removed
.cargo/config.toml              static CRT for x86_64-pc-windows-msvc (unchanged);
                                TS_RS_EXPORT_DIR removed
crates/
  fm-core/                      + watch.rs: FileWatcher (moved from tarpack/watch.rs)
  fm-tarpack/                   + src/session/: Core, DropJob, BuildJob, ProgressCoalescer,
                                  session types (moved from src-tauri tools/tarpack)
  fm-schedule/                  + src/session/ (from src-tauri tools/schedule)
  fm-pipeline/                  + src/session/ (from src-tauri tools/pipeline)
  fm-dbmanager/                 session/ already exists (M6)
apps/filemanager/               package and binary: filemanager (portable exe)
  Cargo.toml, build.rs          build.rs embeds the icon and app manifest (resource
                                compiler; no runtime DLL)
  assets/icons/*.svg            the 16 px icon set, ported from app/icons.tsx
  src/main.rs                   #![windows_subsystem = "windows"]; calls filemanager::run()
  src/lib.rs                    app init, window options (1000×700, min 800×560)
  src/platform/                 trait Platform: open/save dialogs, open in system
                                editor, reveal in Explorer, clipboard. Real impl
                                (GPUI) and a scripted fake (tests, screenshots)
  src/jobs.rs                   runs a Send job on a background thread and delivers
                                progress and the result to an entity on the main thread
  src/app/                      shell: window root, sidebar, dev mode, close guard,
                                external-drop routing
  src/theme/                    tokens.rs (normative token values: replaces tokens.css),
                                the gpui-kit Theme mapping, system light/dark
  src/widgets/                  shared widgets over gpui-kit: buttons, banner,
                                confirm dialog, drop overlay, menu button, shortcuts
                                help, icons, middle-truncated path, tri-state checkbox
  src/tools/mod.rs              the tool registry (one list)
  src/tools/<tool>/backend.rs   trait <Tool>Backend: one method per former command
  src/tools/<tool>/model.rs     Entity holding the backend; starts jobs; notifies views
  src/tools/<tool>/view/        the tool's views
  src/fixtures/                 fixture backends for screenshots and view tests
                                (feature "fixtures"; not in the release exe)
  src/bin/shoot.rs              the screenshot harness (feature "fixtures")
docs/manual/screenshots/        README.md only: how to run the harness
```

The new app is built **beside** `apps/desktop` (package `filemanager-gpui`,
binary `filemanager-gpui`) until the cutover milestone (M9), which deletes
`apps/desktop` and renames the package and binary to `filemanager`. Until
then the Tauri exe is the shipping one.

### 3.2 Where the session logic goes

Today each tool's session logic is in `apps/desktop/src-tauri/src/tools/<tool>/core.rs`
as plain functions with no Tauri types, and its snapshot types in `types.rs`.
They move into a `session` module of the tool's **own domain crate**
(`fm-tarpack::session`, `fm-schedule::session`, `fm-pipeline::session`),
with their tests. `fm-dbmanager::session` already follows this shape.

Reasons: one crate per tool keeps "tools never import each other"; the logic
stays testable without any UI; a shared `fm-sessions` crate would import every
tool. The Tauri commands become thin wrappers over the moved code in M2, so
the move is proved by the existing tests and the unchanged generated TS
before any GPUI code exists.

### 3.3 How views call the domain

- A tool's **model** (`tools/<tool>/model.rs`) is a GPUI entity that owns its
  backend and the latest session snapshot. Views read the snapshot and call
  model methods; they never touch the filesystem or a domain crate directly.
- The **backend trait** (`tools/<tool>/backend.rs`) has one method per
  former Tauri command. The real implementation wraps the domain crate's
  `session::Core`; the fixture implementation returns canned snapshots (the
  equivalent of today's `docs/manual/screenshots/mock.js`). This is what
  makes screenshots and view tests run without the filesystem.
- **No mutex**: the model owns `Core` on the main thread. The existing
  begin / `Job::run` / finish split (tarpack `DropJob`, `BuildJob`; pipeline
  `Job`) stays: begin and finish run on the main thread; `run` runs on a
  background thread via `jobs.rs`.
- Domain types are used directly: no string-encoded integers, no serde for
  the UI, no JSON. `#[derive(Serialize)]` stays only where persistence needs it.

### 3.4 Background work, progress and file watching

- `jobs.rs` spawns a `Send + 'static` job on a dedicated thread (long,
  blocking work: archive builds, folder walks, pipeline steps) and returns a
  GPUI task that awaits the result and calls the model's finish on the main
  thread. Short work may use GPUI's background executor.
- Progress: the job's progress sink (tarpack's `ProgressCoalescer`,
  unchanged, 50 ms) sends into a channel; a foreground task drains it into
  the model and calls `notify`.
- File watching: `fm_core::watch::FileWatcher` (notify-debouncer-mini, 300 ms,
  parent-directory watch, moved from `tools/tarpack/watch.rs`) calls an
  `Fn() + Send` callback on its thread; the callback sends into a channel
  that a foreground task turns into a model update (tarpack "changed on
  disk"; later the dbmanager config reload).

### 3.5 The typed boundary disappears

Removed in M9: ts-rs (workspace dependency and every derive),
`generated_types.rs`, `lib/generated/`, `lib/*.ts`, the currency test, the
`TS_RS_EXPORT_DIR` setting, the `.gitattributes` line, the Tauri command
layer, `tauri.conf.json`, the CSP, the capability file and its test, and the
`tools::register` single-`invoke_handler` rule and test.

The invariant "a type that crosses the core/UI boundary is defined once and
mirrored" is replaced by: **views consume the domain crates' session types
directly, through the tool's model and backend trait; there is no mirror.**

The new **core/UI boundary** is the backend trait and model of each tool
(`apps/filemanager/src/tools/<tool>/{backend,model}.rs`) plus
`src/platform/` and `src/jobs.rs`. The backend `implementer` owns those,
`crates/**`, `src/fixtures/`, the shoot harness and CI. The `ui-implementer`
owns `src/theme/`, `src/widgets/`, `src/app/` views and
`src/tools/<tool>/view/`, and writes screenshot scenarios against the
harness API.

### 3.6 Tests

- Domain and session tests stay plain `cargo test` in the crates (temp dirs).
- Model tests use `#[gpui::test]` / `TestAppContext` with the fixture or real
  backend (real only inside a temp dir) and a fake `Platform`.
- View tests use the visual test context (simulated keystrokes and clicks on
  debug selectors) to prove wiring, not pixels.
- Today's ~395 vitest tests are ported by **behaviour**: each tool milestone
  lists every case in its `*.test.tsx` files as ported, or dropped with a
  reason (axe, ARIA attributes, DOM structure, CSS class).

## 4. Design-system port *(details are the ui-designer's)*

- `tokens.css` is replaced by `apps/filemanager/src/theme/tokens.rs`: one
  struct per theme (light, dark) with every token of today by the same name
  (`bg`, `surface`, `surface_sunken`, `border`, `text`, `text_muted`,
  `accent`, `accent_text`, `ok`, `warn`, `danger`, `focus_ring`,
  `drop_overlay`, `shadow_overlay`, `scrim`; `space_1..6`; `radius`;
  `font_ui`, `font_mono`; `font_size_sm/md/lg/xl`). It becomes the normative
  source `DESIGN.md` cites. Values are copied, not redesigned.
- The tokens are mapped onto gpui-kit's theme colours, so gpui-kit
  components render in our palette; tokens gpui-kit has no slot for are read
  from our own global.
- Sizes stay in rem (GPUI has `rems()` against the window's rem size;
  1rem = 16 px as today).
- The theme follows the system appearance (light/dark) live, with no toggle.
- A token test replaces `tokens.test.ts`: the contrast pairs asserted today
  stay asserted (a cheap guard, not an accessibility gate).
- Fonts: Segoe UI and Cascadia Mono with fallbacks, as today. Whether to
  embed Cascadia Mono (OFL) for deterministic screenshots is Q7.

## 5. Docs and screenshot pipeline

- `src/bin/shoot.rs` (feature `fixtures`) replaces `shoot.js`/`lib.js`/`mock.js`.
  It opens the real app window with fixture backends and a scripted
  `Platform`, plays a scenario (select tool, click by debug selector, press
  keys, inject a watcher event or an external drop, wait until idle),
  renders, and writes `docs/manual/images/<name>.png`. It takes an optional
  name prefix, as `shoot.js` does.
- Window sizes as today: 1366×800 for the Tar Packager, 1000×700 for the
  others, scale factor 1.
- The capture method is chosen by the spike (§8, item B6). Windows-only (Windows 11, D9):
  the screenshots are generated on Windows (CI and the developer's machine),
  not on Linux as today.
- CI runs the harness on the Windows job and uploads the PNGs as an artifact.
  Whether CI also fails when committed images differ is Q8.
- Scenario names stay the same as today's (`docs/manual/images/*.png`), so
  `docs/manual/user-manual.md` needs no image renames.

## 6. CI after the migration

- **windows** job, on a runner pinned to `windows-2025` (Windows Server
  2025, which shares the Windows 11 24H2 code base; GitHub has no x64
  Windows 11 client image, so Windows 11 itself is covered by the human's
  manual checks): `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked` (including `#[gpui::test]`),
  `cargo build --release -p filemanager --locked`, the `dumpbin` check
  (forbidden list unchanged), the screenshot harness and upload, and the
  `FileManager-<version>-x64.exe` artifact, with the version now read from
  `cargo metadata` (no `tauri.conf.json`).
- **linux** job: `cargo fmt --all --check`, and clippy + tests for at least
  the domain crates. Whether GPUI builds and tests on Linux CI (and which
  system packages it needs) is a spike result (item A9); if not, the linux
  job is scoped to `--exclude filemanager`.
- Removed: `actions/setup-node`, `npm ci`, the frontend build, typecheck,
  lint, vitest, the webkit2gtk packages.
- The "fm-* must not depend on tauri" check becomes "must not depend on
  `gpui`, `gpui-pre` or `gpui-component`", for all five `fm-*` crates.

## 7. Agent roles, `impeccable`, `CLAUDE.md`

These change in M10, **after the human approves** them (Q1–Q3):

- **`impeccable`** is a web skill (`audit`, `polish`, detectors run on
  HTML/CSS). Proposal: keep the plugin for `shape` (design briefs) and
  `critique` (of generated screenshots), and drop `audit`, `polish` and the
  detector from the role definitions. `PRODUCT.md` "Platform: web" becomes
  "desktop (native, GPUI)". Q2.
- **ui-designer**: writes the theme mapping and component inventory against
  gpui-kit; a11y acceptance becomes keyboard + visible label criteria.
- **ui-implementer**: owns `theme/`, `widgets/`, `app/` views,
  `tools/*/view/`; proves work with `#[gpui::test]` view tests and
  regenerated screenshots compared to the parity checklist; no `npx`.
- **implementer**: owns crates, models, backend traits, platform, jobs,
  fixtures, harness, CI.
- **reviewer**: drops axe/jsx-a11y as P0; checks keyboard operation and
  labels per the new invariant; checks screenshots against the parity list.
- **planner**: its "accessibility is a requirement" rule follows the new
  invariant.
- `.claude/hooks/task-plan-only.mjs` runs under Node. It is agent tooling,
  not app tooling. Keep it, or port it to a shell script so the repo has no
  Node at all: Q3.
- Each `.claude/agents/*.md` change is mirrored in `.codex/agents/*.toml`.

## 8. The spike (M1) — go/no-go gate

Time-boxed, throwaway, on branch `spike/gpui-migration`, in a standalone
workspace `spike/gpui/` with its own workflow, never merged to `main` (D6). Two task plans,
run in order by the backend `implementer`; the themed screen is a
transcription of existing values, not design work.

- [`M1-spike-a-platform.md`](../tasks/gpui-migration/M1-spike-a-platform.md):
  build, exe, platform integration, headless tests, CI.
- [`M1-spike-b-ui.md`](../tasks/gpui-migration/M1-spike-b-ui.md):
  gpui-kit widgets, the themed Pipeline screen, PNG capture, the report.

Items, criteria and what failure means (full detail in the task plans):

| Id | Item | Pass | Class |
|---|---|---|---|
| A1 | Portable exe, `+crt-static`, linking all five `fm-*` crates | `dumpbin /dependents` lists none of `vcruntime*`, `msvcp*`, `api-ms-win-crt-*`, `zstd*`, `liblzma*`, `sqlite3*`, `WebView2Loader.dll`; every listed DLL is a Windows system DLL; one file runs from a USB-style folder | Hard |
| A2 | Native open dialog (file, folder) and save dialog with a suggested name and an extension filter | Returns `PathBuf`s; a non-UTF-8 (lone-surrogate) name round-trips; filter works (GPUI's own API or a crate such as `rfd` beside GPUI) | Hard |
| A3 | Files and folders dropped from Explorer | Drag-over and leave are observable (for the overlay); drop delivers every path exactly | Hard |
| A4 | Window close interception | Close (X, Alt+F4) can be held, a prompt shown, then cancelled or completed programmatically | Hard |
| A5 | Open a file in the system editor; reveal in Explorer | Default handler opens; Explorer opens with the file selected | Soft (Win32 fallback is cheap) |
| A6 | `#[gpui::test]` headless tests on Windows CI | Model and view tests (keystrokes into a gpui-kit input, a click) pass on `windows-2025`, 3 runs, no flake | Hard |
| A7 | Background job + progress + file watcher reach the UI | Progress at ≤ 50 ms granularity without blocking input; a watcher event updates the view | Hard |
| A8 | Exe size, clean release build time, incremental debug rebuild time, toolchain/MSRV, licenses | Measured and reported, no thresholds (D8); no GPL/AGPL/LGPL dependency | Soft (report only) |
| A9 | Linux build and test | Reported: builds or not, packages needed | Soft |
| A10 | clippy `pedantic` with `-D warnings` on GPUI code | Clean with at most a few scoped `#[expect]`s, none crate-wide | Hard |
| B1 | gpui-kit text input | Typing, paste, selection, placeholder, read-only, invalid border, mono font, Enter event | Hard |
| B2 | Checkbox incl. "not set" (indeterminate) | gpui-kit supports it, or a custom tri-state widget is built in the spike, keyboard-toggleable | Hard (custom allowed) |
| B3 | Virtual list / table | 10,000 rows scroll smoothly; fixed rem columns; custom cells (icon + word); keyboard row focus Up/Down; sticky header | Hard |
| B4 | Modal confirmation | Scrim, focus starts on Cancel, Escape cancels, focus returns, busy state keeps it open and inert | Hard |
| B5 | Menu and popover | A Recent-style menu (keys Up/Down/Escape) and a non-modal popover, both themable with our shadow | Soft (custom allowed) |
| B6 | Theming: today's Pipeline "folder chosen" screen in light and dark from the tokens | Sampled colours equal the tokens (±1 per channel); region geometry within ~2 px of `docs/manual/images/pipeline-chosen.png`; text rendering may differ (D7); follows the system theme live | Hard |
| B7 | Screenshot to PNG unattended | A command renders the B6 screen to PNG at scale 1 on `windows-2025`, identical across two runs; also works on a developer's Windows machine | Hard (local-only = partial; human decides) |
| B8 | Keyboard and labels, as found (incl. the Ctrl+Alt+Shift+D chord) | Reported per widget; no extra work | Soft |

**Gate.** All Hard items pass → go: the planner writes M2+ task plans, the
ui-designer writes the UI plan. Any Hard fail → no-go by default; the
report says what would change the answer, and the human may override. Soft
items are weighed by the human. Manual items (dialogs, Explorer drop, close,
editor, live theme switch) are verified by the human on Windows 11 (D9) with the
CI exe, using the checklist the spike writes, because the implementer has no
interactive Windows desktop.

## 9. Milestone index

| # | Task plan | Goal | Depends on | UI plan? |
|---|---|---|---|---|
| M1a | [`M1-spike-a-platform.md`](../tasks/gpui-migration/M1-spike-a-platform.md) | Spike: exe, platform, tests, CI | none | no |
| M1b | [`M1-spike-b-ui.md`](../tasks/gpui-migration/M1-spike-b-ui.md) | Spike: widgets, theme, screenshots, report | M1a | no (transcription only) |
| G | — | **Human go/no-go on the spike report** | M1b | — |
| M2 | later | Session layer extraction | G | no |
| M3 | later | GPUI app skeleton and plumbing | M2 | contract for U-shell |
| M4 | later | Screenshot harness | M3 | contract for U scenarios |
| M5 | later | Pipeline model and backend | M3 (M4 for its shots) | **depends on UI plan** (view half) |
| M6 | later | Schedule Creator model and backend | M5 | **depends on UI plan** |
| M7 | later | Tar Packager model and backend | M6 | **depends on UI plan** |
| M8 | later | Manual and screenshots cutover | M5–M7 + their U tasks | **depends on UI plan** |
| M9 | later | Cutover: remove Tauri, React, npm, ts-rs; CI cleanup | M8 | no |
| M10 | later | `CLAUDE.md`, docs, agent roles, invariants | M9, Q1–Q3 answered | partly (DESIGN.md/PRODUCT.md are the ui-designer's) |

Brief for each post-spike milestone:

**M2 — Session layer extraction.** Move `src-tauri/src/tools/{pipeline,schedule,tarpack}/{core,types}.rs`
and `tarpack/progress.rs` into `fm-pipeline::session`, `fm-schedule::session`,
`fm-tarpack::session` with their tests; move `tarpack/watch.rs` to
`fm_core::watch::FileWatcher` (this absorbs the FileWatcher half of dbmanager
M13). The Tauri commands become thin wrappers over the moved code. ts-rs
derives move with the types; `generated_types.rs` imports from the crates.
*Acceptance:* `lib/generated/` is byte-identical without regenerating; every
moved test passes in its new crate; the shell's `tools/*/` contain only
command glue; fmt/clippy/test green on both CI jobs; no `fm-*` crate gains a
tauri dependency.

**M3 — GPUI app skeleton and plumbing.** `apps/filemanager` (package
`filemanager-gpui`): pinned gpui + gpui-component as workspace deps, window
(1000×700, min 800×560, title, icon), `Platform` trait with real and fake
impls, `jobs.rs`, external-drop routing to the active tool, the close-guard
registry (tools report unsaved work; the shell holds the close and asks the
UI to prompt), the developer-mode action (Ctrl+Alt+Shift+D, off at launch),
the tool registry, an unstyled placeholder shell. CI builds the new exe on
Windows and runs the dumpbin check on it too. *Acceptance:* model-level
`#[gpui::test]`s for jobs (progress order, finish on main thread), drop
routing, close guard (held, cancelled, completed), dev-mode toggle; both
exes pass dumpbin. *The U-shell tasks (theme, shared widgets, sidebar) start
after M3.*

**M4 — Screenshot harness.** `src/bin/shoot.rs` and the scenario API
(open, select tool, click/keys by debug selector, inject drop and watcher
events, script dialog answers, wait idle, capture), using the capture method
the spike proved; a fixture-backend pattern; CI step on Windows uploading
PNGs. *Acceptance:* one sample scenario renders the shell; two runs give
identical PNGs; the harness is not in the release exe.

**M5 / M6 / M7 — one per tool (Pipeline, Schedule Creator, Tar Packager),
in that order (smallest first).** Each: `backend.rs` (trait, real impl over
the crate's `session::Core`), `model.rs`, fixture backend reproducing every
`mock.js` scenario for the tool, model tests porting the behaviour of the
tool's `*.test.tsx` files (with a ported/dropped list), and for the tar
packager: build progress, overwrite confirmation, manifest watcher, drop
matching off the main thread, recent list, editor/reveal, create from
example, dev-mode gating. The tool's U tasks (views + screenshot scenarios)
run after its M and must reach the ui-designer's parity checklist.
*Acceptance:* behaviour parity with today (every listed case), the tool's
screenshots regenerate, fmt/clippy/test green.

**M8 — Manual and screenshots cutover.** Every image in
`docs/manual/images/` regenerated by the harness; `user-manual.md` updated
where wording or visuals changed (e.g. no "browser" wording);
`docs/manual/screenshots/` keeps only a README for the Rust harness;
`shoot.js`, `lib.js`, `mock.js` deleted. *Acceptance:* every image the
manual references is produced by a named scenario; the human has reviewed
the side-by-side set.

**M9 — Cutover and removal.** Delete `apps/desktop` (Tauri shell, React
frontend, `lib/`, `lib/generated/`, `generated_types.rs`, `tauri.conf.json`,
capabilities, icons moved to `apps/filemanager`), root `package.json`,
`package-lock.json`, `.gitattributes` generated line, ts-rs from the
workspace and every crate, `TS_RS_EXPORT_DIR`; rename the package and
binary to `filemanager`; CI per §6. *Acceptance:* `git ls-files` contains
no `package.json`, `*.ts`, `*.tsx`; `cargo tree` has no tauri or ts-rs;
the artifact `FileManager-<version>-x64.exe` passes dumpbin; both CI jobs
green.

**M10 — Docs and governance.** `CLAUDE.md` (status incl. Windows 11 only per D9, layout, commands,
invariants per §10, "Adding a tool"), `docs/pipeline.md`, `docs/schedule.md`
(no `lib/generated` references), `.env.example` unchanged, agent roles per
§7 in both runtimes, `.claude/settings.json` per Q2, the hook per Q3. The
ui-designer updates `DESIGN.md` (normative source `theme/tokens.rs`) and
`PRODUCT.md` (platform, Windows 11 only per D9, accessibility section). *Acceptance:* no doc names
npm, Tauri, ts-rs, `lib/*.ts` or `tokens.css` except as history; every
`.claude` role matches its `.codex` twin.

## 10. Cross-cutting rules for every post-spike task plan

1. The `fm-*` crates never depend on gpui, gpui-pre or gpui-component.
2. Views never touch the filesystem or call a domain crate directly; they
   go through the tool's model and backend trait.
3. Tools never import each other; shared code goes in `fm-core` (domain) or
   `apps/filemanager/src/{widgets,platform,jobs}` (UI).
4. gpui and gpui-component are pinned at exact versions in
   `[workspace.dependencies]`; an upgrade is its own reviewed change.
5. Destructive operations stay confirmed or recoverable, and batch failures
   are reported: overwrite of an archive, apply to an XML, pipeline steps
   that change files, dbmanager save/delete, closing with unsaved edits.
6. Tests that touch the filesystem run in a temp dir; screenshots use fixture
   backends, never real files.
7. Paths stay `Path`/`PathBuf`/`OsString`; only rendering converts, lossily.
8. fmt and clippy `pedantic` with `-D warnings`; scoped `#[expect]` only.
9. Keyboard operation and visible labels are kept where gpui-kit provides
   them (pending Q1).

### 10.3 Proposed `CLAUDE.md` invariant changes (human approval required, Q1)

| Today | Proposed |
|---|---|
| "Accessibility is a requirement, not a polish item. Every interactive element is keyboard reachable and labelled." | "Keyboard operation is expected: every action has a keyboard path and every control a visible label or tooltip, using what gpui-kit provides. Screen-reader support, high contrast and text scaling are not requirements." |
| Boundary types defined once and mirrored | Removed; views use domain session types through the tool's model (§3.5). |
| `lib/generated/` and ts-rs rules | Removed. |
| `tools::register` is the only `invoke_handler`/`setup` caller | Removed; tools are added to the single list in `src/tools/mod.rs`. |
| Webview CSP is a reviewed decision | Removed (no webview). |
| Core/UI boundary is `apps/desktop/src/lib/` | Boundary is each tool's `backend.rs`/`model.rs` plus `platform/` and `jobs.rs`. |
| `fm-*` never depend on tauri | `fm-*` never depend on gpui or gpui-component. |

Until the human approves, the current invariants bind every task plan.

## 11. The Database Manager under the migration

**Unchanged:** M7 (save), M8 (links), M9 (delete) in `fm-dbmanager`. They run
whenever they are approved, independent of the spike. If they land before
M9 of this plan, they keep deriving `ts_rs::TS` per today's rules; M9
removes the derives.

**Changes, applied to the dbmanager plans after the spike is accepted** (not
yet made):

| Task | Fate | Notes |
|---|---|---|
| M10 Tauri contract | **Replaced** by a GPUI model + backend trait milestone | No commands, no `lib/dbmanager.ts`, no generated types; snapshot types stay Rust; integers stop being strings; `spawn_blocking` becomes `jobs.rs`. |
| M11 TS validation mirror | **Dropped** | The UI calls `fm_dbmanager::rules::validate` directly while typing. `validation-cases.json` stays only if the Rust tests still want it. |
| M5's pattern-dialect restriction | **Lift, optionally** (new small milestone) | With only Rust `regex` running, the shorthand-class ban is unnecessary. Loosening only: existing configs stay valid. Human decision, Q10. |
| M12 Docs, manual, Windows check | **Rewritten** | Screenshots via the Rust harness; runs after this plan's M8. |
| M13 Config watch and Edit config | **Shrunk** | The shared `FileWatcher` lands in this plan's M2; M13 keeps the config reload (model notification instead of an event) and Edit config through `Platform`. |
| M14 Window-close guard | **Dropped** | Absorbed by this plan's M3 close-guard registry; no capability file exists. |
| U1 Shared controls | **Merged** into this plan's shared-widget U tasks (danger button, extended confirm dialog, menu button, shortcuts help, icons, form controls, tri-state checkbox) | |
| U2–U11 | **Rewritten for GPUI** | Design decisions in `dbmanager-ui.md` (layout, interaction, copy) carry over; files, tests and acceptance are redone. Container-query breakpoints become width-based layout. U5 no longer depends on M11. |
| U12 Keyboard and audit | **Reduced or dropped** | Keep keyboard map and shortcuts help (fold into U2–U11); drop the a11y sweep, axe, `/impeccable audit`, the 200% text pass. Q9. |
| U13 Close guard | **Shrunk** | Registers dbmanager's unsaved drafts with the shell's guard. |

**Order:** dbmanager M7–M9 any time. The GPUI dbmanager milestones
(model, shrunk M13, U2–U11, U13) start after this plan's M3, M4 and the
shared-widget U tasks; recommended after M7 (tar packager) so the patterns
have settled, and no later than right after M9. dbmanager M12 runs after
this plan's M8.

## 12. Risks

1. **Churn.** gpui-kit pins a GPUI snapshot that changes weekly; upgrades
   will break code. Mitigation: exact pins, deliberate upgrade milestones,
   a thin widget layer so gpui-kit API changes stay in `widgets/`.
2. **Single publisher / supply chain** for `gpui-pre` and `gpui-component`.
   Mitigation: `Cargo.lock` committed, `--locked` in CI; consider vendoring
   (Q11).
3. **Parity gaps.** CSS features with no direct GPUI equivalent: container
   queries (reflowing table below 64rem), middle truncation of paths, sticky
   table header inside a view-level scroll, dashed 2 px drop border,
   selectable read-only text (the extraction command), `tabular-nums`. Each
   needs a widget or an accepted difference; the ui-designer's parity
   checklist decides.
4. **Screenshot determinism and fonts.** Today's shots are rendered in
   Chromium on Linux with Inter; new ones in DirectWrite with Segoe UI, so
   every image changes once. Cascadia Mono may be missing on CI images.
5. **GPU-less CI.** GPUI's DirectX renderer must run on WARP on
   `windows-2025`, or screenshots and real-window tests become local-only.
6. **Build time and exe size** grow; LTO with `codegen-units = 1` on a GPUI
   tree may be slow.
7. **Test rewrite cost.** ~395 frontend tests are ported by behaviour; some
   coverage (a11y, DOM semantics) is deliberately lost.
8. **Clippy pedantic** on render code may be noisy (`too_many_lines`,
   `needless_pass_by_value`, macro-generated code).
9. **Two apps during the migration** (M3–M8): the Tauri exe ships; changes to
   tool behaviour in that window must be made in the session layer (M2) so
   both apps get them.
10. **CI is not Windows 11.** The hosted `windows-2025` runner is Windows
    Server 2025 (Windows 11 24H2 code base), not the client OS. Client-only
    behaviour (Explorer drag, theme switching, fonts such as Cascadia Mono)
    is proved only by the manual checklist on Windows 11.
11. **Sunk work:** `DESIGN.md`'s web specifics, the dbmanager UI plan's
    implementation details, and the `impeccable` audit flow.

## 13. Handoff to ui-designer

```
HANDOFF: ui-designer — GPUI migration (gpui-migration)

When
  Only after the human accepts the spike report (M1, go). Read the report on
  branch spike/gpui-migration at spike/gpui/REPORT.md and the themed spike
  screen it shows; it records which gpui-kit components work and which
  gaps needed custom widgets.

Request
  Port FileManager's existing UI to GPUI + gpui-kit so it looks similar to
  today: same layout, tokens, components and states (DESIGN.md,
  apps/desktop/src/styles/tokens.css, styles/*.css, the React views). Not a
  redesign. Write docs/plans/project/gpui-migration-ui.md and the U<n> task
  plans under docs/plans/tasks/gpui-migration/.

Must produce
  1. Theme mapping: every token in tokens.css (light and dark) to a field of
     apps/filemanager/src/theme/tokens.rs, and each gpui-kit theme colour
     slot to a token (or "unused"). Shadow and scrim values. Fonts and
     fallbacks. rem base. Live system light/dark.
  2. Component inventory: every shared and tool-local React component
     (app/*.tsx, tools/*/*.tsx) mapped to a gpui-kit component, a wrapper in
     apps/filemanager/src/widgets/, or a custom widget, with its states
     (hover, active, focus, disabled, busy, read-only, invalid, current).
     Include DESIGN.md's planned dbmanager controls (danger button, extended
     confirm dialog, menu button, shortcuts help, form controls, tri-state
     checkbox, tags).
  3. Per-tool screen parity checklists (Pipeline, Schedule Creator, Tar
     Packager, shell): regions, geometry, states, copy, keyboard map, and
     the accepted differences (risk 3 in §12: reflow, middle truncation,
     sticky header, dashed border, selectable text, tabular numerals).
  4. Screenshot scenario list: every image in docs/manual/images/ (30 today,
     scenarios in docs/manual/screenshots/shoot.js and mock.js) as a named
     scenario with its fixture state and steps; any new ones the manual
     needs.
  5. U task plans: theme + tokens test; shared widgets; shell (sidebar,
     current marker, dev mode surface, close prompt, drop overlay); then the
     views of each tool; each tool's scenarios.
  6. DESIGN.md and PRODUCT.md updates for M10 (normative source
     theme/tokens.rs; platform; accessibility section per Q1).

Backend milestones the UI depends on (ordering)
  - M3 (app skeleton: Platform, jobs, drop routing, close-guard registry,
    dev-mode action, tool registry) lands before ANY U task.
  - M4 (screenshot harness API) lands before any scenario task.
  - M5 / M6 / M7 (each tool's model + backend trait + fixture backend) land
    before that tool's view tasks. Order: Pipeline, Schedule, Tar Packager.
  - M8 consumes all scenario tasks; M9 waits for M8.

Constraints already established
  - Similar, not redesigned (D3). Tolerance (D7): same layout and token
    colours, sampled colours ±1 per channel, geometry within ~2 px; text
    rendering may differ.
  - Light and dark follow the system; no toggle.
  - Accessibility: keyboard paths and visible labels where gpui-kit gives
    them; screen readers not a gate (D5, pending Q1). No axe, no jsx-a11y.
  - Views call only their model; no filesystem or domain calls in views.
  - Confirmations as today: overwrite, apply, changing pipeline steps,
    dbmanager save/delete, close with unsaved edits. Modal only for those.
  - impeccable: shape/critique only, on screenshots (pending Q2).
  - Windows 11 only (D9); scale factor 1 for screenshots; 1000×700 default,
    1366×800 for Tar Packager shots, min window 800×560.
```

## 14. Open questions for the human

Remaining questions keep their original numbers; they stay open until after
the spike.

1. **Accessibility invariant (D5).** Approve the replacement text in §10.3?
   Keep the token contrast test as a cheap guard?
2. **`impeccable`.** Keep the plugin for `shape` and `critique` on
   screenshots and drop `audit`/`polish`, or remove it entirely?
3. **The Node hook.** `.claude/hooks/task-plan-only.mjs` needs Node, which
   Claude Code itself already brings. Keep it, or port it to a shell script so
   the repository has no `.mjs` at all?
7. **Fonts.** Embed Cascadia Mono (OFL) for identical rendering everywhere,
   or rely on the system with a Consolas fallback?
8. **Screenshots in CI.** Upload only (proposed), or fail CI when committed
   images differ?
9. **dbmanager accessibility tasks.** Drop U12's a11y sweep and 200% text
   pass, keeping only keyboard and shortcuts help (proposed)?
10. **dbmanager pattern dialect.** Lift M5's shorthand-class ban once only
    Rust validates (a loosening, compatible), or keep it?
11. **Vendoring.** Vendor `gpui-pre` and `gpui-component` sources, or rely
    on crates.io with a committed lock file?
13. **Linux CI.** If GPUI does not build on the Linux runner cheaply, may the
    linux job cover only the domain crates?

### 14.1 Answered (2026-11-20)

- **Q4 Spike location →** D6: branch `spike/gpui-migration`, standalone
  `spike/gpui/` workspace and workflow, never merged; pushing the branch is
  approved.
- **Q5 Similarity →** D7: same layout and token colours; sampled colours
  ±1 per channel, geometry within ~2 px; text rendering may differ.
- **Q6 Exe size and build time →** D8: no limit; measure and report only
  (A8 is soft and report-only).
- **Q12 Minimum Windows →** D9: Windows 11 only. CI runs on a pinned
  `windows-2025` runner (the closest hosted x64 image); the spike's manual
  checklist is run on Windows 11 and records its build number.
