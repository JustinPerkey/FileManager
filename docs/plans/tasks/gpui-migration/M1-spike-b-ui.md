# M1b — Spike, part B: gpui-kit widgets, the themed screen, PNG capture, the report

Status: awaiting approval (revised 2026-11-20: the human's answers on
similarity, size/build time, minimum Windows version and spike location are
folded in)
Project: gpui-migration   Depends on: M1a (landed on branch `spike/gpui-migration`)

## Goal

Prove or disprove that gpui-kit's widgets cover FileManager's controls, that
today's look can be reproduced from the existing design tokens, and that the
manual's screenshots can be rendered to PNG unattended; then finish the
spike report the human decides on.

## Context

### The spike so far

The human has decided to replace FileManager's Tauri + React frontend with
upstream Zed GPUI and gpui-kit (crate `gpui-component`), removing Tauri,
React, TypeScript, ts-rs and npm, **if** this spike passes. Part A
(`M1a`) built, on branch `spike/gpui-migration`:

- `spike/gpui/`: a standalone Cargo workspace, package and binary
  `fm-spike`, pinned `gpui-component` and the exact GPUI snapshot it
  requires (read the versions from `spike/gpui/Cargo.toml`), linking the five
  `fm-*` crates by path; a check screen for manual items; headless tests.
- `spike/gpui/REPORT.md` with sections A1–A10, and `spike/gpui/CHECKLIST.md`.
- `.github/workflows/spike-gpui.yml`: a windows job (clippy, tests, release
  build, dumpbin, artifacts) and a non-blocking linux job.

Extend those files. Read `REPORT.md` first: if A1 (exe), A3 (drop) or A6
(headless tests) failed, still do this part, but say in the summary that the
gate already failed.

### The human's decisions that bind the spike

- **Similarity:** same layout and token colours. Sampled colours within ±1
  per channel of the tokens, region geometry within about 2 px of today's
  images. **Text rendering may differ** (glyph shapes, antialiasing, exact
  text widths); it is not a failure.
- **Windows 11 only.** CI runs on the pinned `windows-2025` runner (Windows
  Server 2025, Windows 11 24H2 code base; GitHub has no x64 Windows 11 client
  image). The human runs the manual checklist on Windows 11.
- **Exe size and build time have no limit**; part A measures and reports
  them.
- **Location:** branch `spike/gpui-migration`, standalone `spike/gpui/`
  workspace and workflow, never merged. Committing to and pushing that
  branch is approved; no other branch.

### Rules that bind this task

- **No design decisions.** The themed screen is a transcription: every
  colour, size, spacing, radius and font comes from the files named below.
  Where GPUI cannot express a value, record the gap; do not invent a
  substitute style.
- Accessibility is not a gate. Record, without extra work, whether gpui-kit
  widgets are keyboard-operable (Tab, Space/Enter, Escape) and labelled.
- Paths stay `PathBuf`; rendering converts lossily.
- Tests touching the filesystem use temp dirs.
- `cargo fmt` clean; clippy `pedantic` with `-D warnings`; scoped
  `#[expect]` only, counted in the report.
- Only `spike/gpui/**` and `.github/workflows/spike-gpui.yml` change.
- Same time box as part A: about three working days; per item, gpui-kit
  first, then at most one fallback (named per item); if it still fails,
  record FAIL with what was tried. Do not patch GPUI or gpui-kit.

### The design source (read in the repository, on the spike branch)

- `apps/desktop/src/styles/tokens.css`: every token, light and dark. Light:
  `--bg #f7f7f8`, `--surface #ffffff`, `--surface-sunken #eef0f2`,
  `--border #d9dce1`, `--text #1b1d21`, `--text-muted #5b616b`,
  `--accent #2457c5`, `--accent-text #ffffff`, `--ok #1d7a46`,
  `--warn #8a5a00`, `--danger #b3261e`, `--focus-ring #2457c5`. Dark:
  `--bg #16171a`, `--surface #1e2024`, `--surface-sunken #121316`,
  `--border #33363d`, `--text #e8e9ec`, `--text-muted #a3a8b1`,
  `--accent #7aa2ff`, `--accent-text #0d1530`, `--ok #5fd08f`,
  `--warn #f2c14e`, `--danger #ff8a80`, `--focus-ring #7aa2ff`. Spacing
  `--space-1..6` = 4, 8, 12, 16, 24, 32 px; `--radius` 6 px; type sizes sm
  0.8125rem, md 0.875rem (body), lg 1rem, xl 1.25rem (title), 1rem = 16 px;
  `--font-ui` "Segoe UI", system-ui; `--font-mono` "Cascadia Mono",
  Consolas; `--shadow-overlay` `0 8px 24px rgb(0 0 0 / 0.18)` light,
  `0.5` dark; `--scrim` `rgb(0 0 0 / 0.4)` light, `0.6` dark. The file is
  authoritative if this excerpt disagrees.
- `apps/desktop/src/styles/base.css`, `app.css` (shell: sidebar 12rem,
  max 40%, `--surface-sunken`, 1 px right border, padding
  `--space-3 --space-2`; nav items 8×12 px, current item `--surface` fill,
  1 px border, weight 600, a 6 px `--accent` dot before the label),
  `controls.css` (buttons: primary `--accent` fill, secondary `--surface`
  with `--border`, disabled `--text-muted` on `--surface-sunken` at 55%
  opacity; min 24×24, padding `--space-1 --space-3`), `pipeline.css`
  (the Pipeline view).
- `apps/desktop/src/tools/pipeline/PipelineView.tsx` and
  `apps/desktop/src/app/{AppShell,ToolNav,Button,icons}.tsx`: structure,
  copy and icons of the screen.
- **Reference image:** `docs/manual/images/pipeline-chosen.png`, 1000×700,
  scale 1. It was rendered by Chromium on Linux with Inter and DejaVu Sans
  Mono substituted for Segoe UI and Cascadia Mono, so **text shapes will
  differ**; colours and box geometry should not. Its scenario: folder
  `C:\Deliveries\2026-10` chosen, "12 subfolders", primary button
  "Run next: Check required files", secondary "Run all…", four steps
  (Check required files: Not run, Run enabled; Run batch files, Schedule,
  Pack archives: Waiting, "Run…" disabled, each with "Runs after <previous>
  passes."), the Pipeline item current in the sidebar beside Tar Packager
  and Schedule Creator. There is no dark reference image.

## Items, with pass criteria and what failure means

**B1 — Text input (gpui-kit).** Pass: typing, paste (Ctrl+V), selection,
caret movement, placeholder in `--text-muted`, a read-only state that is
still selectable, an invalid state with a `--danger` border, the mono font,
min height 2rem, and an Enter/submit event the model sees. Fail means:
no-go; every tool has path inputs, the Database Manager has typed fields.

**B2 — Checkbox with "not set".** Pass: gpui-kit's checkbox supports an
indeterminate state, or a custom tri-state widget (unchecked / checked /
not set, 1rem square, `--accent` when checked, the words "Not set" beside
the indeterminate state) is built from GPUI primitives in the spike,
focusable and toggled by Space and click. Record lines of code if custom.
Fail means: no-go only if neither works (the Database Manager needs it).

**B3 — Virtual list / table.** A gpui-kit table (or virtual list) with
10,000 rows of Tar-Packager-like data: Status (icon + word in a status
colour), File, Windows location (mono), Linux target (mono), Mode (mono),
Owner; fixed column widths in rem (Status 7.25, Mode 8.25, Owner 5); 1 px
row rules; header that stays visible while scrolling; hover
`--surface-sunken`; keyboard Up/Down moves a focused row and scrolls it into
view. Pass: all of these, and scrolling a 10,000-row list stays smooth
(no visible stalls; report frame timing if GPUI exposes it). Record whether
rows can have variable height (today's table stacks each row into lines
below a 64rem width) and whether text can be middle-truncated (today paths
keep drive and file name). Fail means: no-go if virtualization, custom cells
or keyboard row focus are missing; the reflow and middle-truncation results
are inputs for the design plan.

**B4 — Modal confirmation.** gpui-kit dialog over a scrim (`--scrim`):
title, short description, a scrolling content region, Cancel and a confirm
button (normal and danger variants). Pass: focus starts on Cancel; Escape
cancels; focus returns to the opener; a busy state (confirm running) keeps
the dialog open, Cancel and Escape inert; the scrim and shadow take our
tokens. Fail means: no-go if a modal with focus control cannot be built;
every destructive operation is confirmed this way.

**B5 — Menu and popover.** A "Recent" menu button (chevron icon) opening a
list of items with a second muted line; Up/Down move, Escape closes and
refocuses the trigger. A non-modal popover anchored under a button
(shortcuts help), closed by Escape. Both on `--surface`, 1 px `--border`,
`--radius`, `--shadow-overlay`. Pass: both work with gpui-kit or a small
custom wrapper. Fail means: soft; record the custom cost.

**B6 — Themed screen.** Rebuild today's Pipeline "folder chosen" screen
(sidebar + Pipeline view, the reference above) in GPUI with gpui-kit
controls, themed from a Rust transcription of the tokens
(`spike/gpui/src/tokens.rs`, light and dark structs, every token by its CSS
name in snake_case), mapped onto gpui-kit's theme colour slots, plus our own
values where gpui-kit has no slot. Icons: transcribe the SVG paths used by
the screen from `apps/desktop/src/app/icons.tsx` into asset files.
The theme follows the Windows light/dark setting, live (switch it while
the app runs; part of the human's checklist).
- Pass, measured on the B7 PNG at 1000×700, scale 1:
  - colours sampled at no fewer than 12 named points (window ground,
    sidebar, sidebar border, current nav fill, accent dot, input fill,
    input border, primary button fill, primary label, secondary button
    border, disabled button, row rule, muted text) equal the token values,
    after the stated opacity, within ±1 per channel, in light and in dark;
  - the bounding boxes of the sidebar, the title, the input, the two
    buttons, and the four step rows are within ~2 px of the reference
    image's in x, y, width and height (measure the reference by pixel
    inspection; record both sets);
  - a side-by-side PNG (reference | GPUI light | GPUI dark) is in the
    artifacts.
- Write a small Rust test or binary that does the sampling and the box
  comparison so it is repeatable; boxes may come from GPUI debug bounds.
- Fail means: no-go if tokens cannot be applied to gpui-kit's controls
  without forking it. A sampled colour off by more than ±1, or a box off by
  more than ~2 px, fails B6 (the human set this tolerance); list each miss
  with its cause and the fix it would need, so the human can judge whether
  it is fixable in the migration. Differences in text rendering alone never
  fail B6.

**B7 — Screenshot to PNG, unattended.** A command,
`fm-spike --shoot <dir> [--dark]`, opens the B6 screen with fixture data
(no filesystem: the "12 subfolders" and step states are canned), waits
until rendered, writes `<dir>/pipeline-chosen.png` (and `-dark`), and
exits. Methods, in order; stop at the first that works and record why the
others were not needed or failed:
1. a GPUI-native render-to-image or offscreen capture, if the pinned
   GPUI has one (search its source for screenshot / capture / render to
   image / visual test);
2. capture of the app's own window via Win32 (`PrintWindow` with
   `PW_RENDERFULLCONTENT`, or Windows.Graphics.Capture, or a read-back of
   the DirectX swap chain), encoded with a pure-Rust PNG encoder;
3. screen capture cropped to the window rectangle (last resort).
- Pass: on `windows-2025` the command produces the PNGs in CI, two
  consecutive runs produce byte-identical images (or report the exact
  pixel difference), uploaded as artifacts; the human confirms it also
  works on their Windows 11 machine, including at a display scale other than
  100% (does the PNG come out at scale 1? If not, record it).
- Record which fonts were used (is Cascadia Mono present on the
  `windows-2025` runner, and on the human's Windows 11 machine?
  if not, what fell back) and whether embedding the font would make
  output identical across machines.
- Fail means: CI-only failure with local success is PARTIAL (the human
  decides whether local-only generation is acceptable); failure everywhere
  is no-go, since the manual's screenshots must stay auto-generated.

**B8 — Keyboard and labels, as found.** No extra work: for each widget
used in B1–B6, record Tab reachability, activation keys and whether it has
a visible label or a tooltip. Also whether a global chord (Ctrl+Alt+Shift+D,
developer mode) can be bound and toggles a flag. Pass: reported.

## The report

Finish `spike/gpui/REPORT.md`:

1. **Versions:** `gpui-component`, GPUI snapshot, Rust, the CI runner image
   (`windows-2025` and its image version), and the Windows 11 edition,
   version and build the human's checklist ran on.
2. **Results table:** every item A1–A10 and B1–B8 with PASS / FAIL /
   PARTIAL / NEEDS-HUMAN, a one-line evidence pointer, and whether it is
   Hard or Soft per this classification: Hard = A1, A2, A3, A4, A6, A7,
   A10, B1, B2, B3, B4, B6, B7; Soft = A5, A8 (report only, no
   thresholds), A9, B5, B8.
3. **Measurements (report only, no thresholds):** exe size, build times, test durations, dependency
   count, licenses, `#[expect]` count, lines of custom widget code.
4. **Gaps found:** every place GPUI or gpui-kit could not do what today's
   UI does, with the workaround used or proposed and its cost.
5. **API notes:** the real names of every API used (dialogs, drop, close,
   open/reveal, tests, appearance, capture), for the migration task plans.
6. **Recommendation:** go / no-go / go-with-conditions, stating the rule:
   all Hard items PASS → go; any Hard FAIL → no-go unless the human
   overrides; NEEDS-HUMAN items are open until the human runs
   `CHECKLIST.md`.

Add B6's live theme switch and B7's local run to `CHECKLIST.md`.

## Files

On branch `spike/gpui-migration` only:

- `spike/gpui/src/**`: widgets demo screens (B1–B5), `tokens.rs` and the
  theme mapping, the Pipeline screen (B6), the `--shoot` path (B7), tests.
- `spike/gpui/assets/**`: transcribed icons.
- `spike/gpui/REPORT.md`, `spike/gpui/CHECKLIST.md`.
- `.github/workflows/spike-gpui.yml`: add the shoot run (twice, compare),
  the colour/geometry check, and upload the PNGs and the side-by-side.

Nothing outside these paths.

## Acceptance criteria

- The spike workflow's windows job produces `pipeline-chosen.png`,
  `pipeline-chosen-dark.png` and the side-by-side as artifacts, or B7 is
  recorded FAIL / PARTIAL with the reason.
- The colour and geometry check runs in CI and its output is quoted in the
  report.
- `REPORT.md` has every section listed above, every item has a status and
  evidence, and the recommendation follows the stated rule.
- `CHECKLIST.md` covers every NEEDS-HUMAN item from both parts.
- fmt, clippy `pedantic` `-D warnings` and tests pass in `spike/gpui`.

## Tests proving completion

From `spike/gpui/`, on Windows CI:

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
target/release/fm-spike --shoot out
target/release/fm-spike --shoot out --dark
```

Named tests (rename to fit): `input_paste_and_submit`,
`tri_state_checkbox_cycles`, `table_keyboard_focus_moves_and_scrolls`,
`dialog_focus_starts_on_cancel_and_escape_cancels`,
`dialog_busy_is_inert`, `menu_keys_wrap_and_escape_refocuses`,
`tokens_match_tokens_css` (parses `apps/desktop/src/styles/tokens.css`
and compares every value with `tokens.rs`), and the B6 colour/geometry
check.

## Out of scope

- The other Pipeline states, other tools' screens, and the manual: they are
  migration milestones after the gate.
- Redesigning anything, or choosing new values.
- Screen-reader, high-contrast or text-scaling support.
- Any change outside `spike/gpui/` and the spike workflow.

## Risks

- If WARP rendering fails on the runner (see part A's report), B7 can only
  be PARTIAL; still produce the local path.
- Text shaping differs from the reference (Inter vs Segoe UI); judge text
  by position and box, not by glyphs.
- gpui-kit's theme may not expose every slot (focus ring, disabled
  opacity, scrim); record each slot you could not set.
