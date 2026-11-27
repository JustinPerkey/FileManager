# U5b — Pipeline view and widget follow-ups from the U5 review

Status: approved (2026-11-23; from the U5 review, which approved U5)
Project: gpui-migration   Depends on: U5 (landed)

## Goal

Close the small follow-ups the U5 review found: tags on banner buttons, an
optional accessible label, a queued focus move while the Pipeline is hidden,
the folder field after a cleaned commit, and long subfolder names.

## Context

U5 built the Pipeline views in `apps/filemanager/src/tools/pipeline/view/`
(`folder.rs`, `run_bar.rs`, `steps.rs`, `tests.rs`, `mod.rs`). It also
edited `src/tools/pipeline/mod.rs`, where `slot()` now passes `window`. The
shared widgets are in `apps/filemanager/src/widgets/` (U2, U3): `FmButton`
(`button.rs`) and `Banner` (`banner.rs`, `.action(button)`).

The screenshot harness clicks and clips only by `.tag("<name>")`
(`src/app/probe.rs`), called last in a builder chain. GPUI debug selectors
work only in `VisualTestContext`.

The model is `PipelineModel` (`src/tools/pipeline/model.rs`, M5); views call
it and never the backend. `crate::app::path::clean_path` trims and unquotes
a typed path.

### What to build

- **(a) Tags on buttons passed to a banner.** `FmButton` cannot carry
  `.tag()` through `Banner::action`, because `action` takes an `FmButton`,
  and `.tag()` wraps the element into another type. Make `Banner::action`
  take any `impl IntoElement`, so a caller passes `button(..).tag("…")`.
  Keep the existing call sites compiling. If changing the signature is
  unsound for the banner's layout, add `FmButton::tag(name)` instead, which
  records the button's own bounds. Say which in the PR. Test: a banner action
  with a tag resolves through `probe::bounds`.
- **(b) Optional accessible label: already done** in U10 as
  `FmButton::accessible_name(text)` (2026-11-26). Use that name; do not add
  a second method. The original wording, for reference: add `FmButton::accessible_label(text)`,
  which overrides the label assistive technology reads, for icon-only
  buttons whose tooltip is not the best name. It is optional: use it only
  where a button's visible label or tooltip is misleading out of context. No
  call site needs it today.
- **(d) A focus move queued while the Pipeline is hidden.** A focus hint
  taken while the Pipeline is not the current tool is applied at its next
  render, so it can steal focus from the shell when the user comes back.
  - Apply a queued focus move only when the Pipeline is the current tool
    **and** focus is inside the Pipeline view or nowhere.
  - Otherwise drop the hint.
  - Test: `queued_focus_is_dropped_when_not_current`.
- **(e) The folder field after a cleaned commit.** When the user commits
  text whose `clean_path` result equals the folder already shown (for
  example the same path with quotes or spaces), nothing is sent. The field
  still shows the raw typed text. Reset the field to the shown folder.
  - Test: `committing_the_shown_folder_with_quotes_resets_the_field`.
- **(f) Long unbreakable subfolder names.** Check that a long subfolder or
  folder name with no break opportunity does not overflow the row or the
  run bar.
  - Use a view test with a session whose folder display path is about 200
    characters with no separators in its last segment. The fixture's
    configurable session may provide it; if `FixturePipeline` cannot
    express it, stop and report the gap (fixtures are the backend
    implementer's).
  - If it overflows, give the text element `min_w_0` and `flex_shrink`, so
    it truncates or wraps inside its cell.
  - Test: `long_folder_name_stays_inside_its_cell` (the element's bounds end
    at or before its container's right edge).

**(c) The mono folder name in the confirmation** is decided in U6's critique,
not here (see U6).

### Rules that bind this task

- Views call only the model. No colour literal outside `theme`. Every
  interactive element stays keyboard reachable and labelled.
- `.tag()` stays the last call of a builder chain.
- fmt (linux job), clippy pedantic `-D warnings` and `cargo test --workspace`
  pass.

## Files

Edit:

- `apps/filemanager/src/widgets/{banner.rs,button.rs}` and their tests;
- `apps/filemanager/src/tools/pipeline/view/{mod.rs,folder.rs,run_bar.rs,steps.rs,tests.rs}`.

Do not touch: models, backends, fixtures, `crates/`, `docs/plans/`.

## Skill

None (no new visual design). Do not run `impeccable`.

## Acceptance criteria

- Every named test passes.
- The existing U2, U3 and U5 tests pass.

## Tests proving completion

- `widgets::banner::tests::tagged_action_resolves` (or the button variant).
- `queued_focus_is_dropped_when_not_current`.
- `committing_the_shown_folder_with_quotes_resets_the_field`.
- `long_folder_name_stays_inside_its_cell`.

## States covered

Folder chosen, hidden and shown again, long names.

## Out of scope

- (c), which is U6's.
- Model changes (report any you need).
