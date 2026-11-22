# M8 — Manual and screenshots from the GPUI harness; the Node screenshot scripts go

Status: approved (2026-11-21). **Depends on the UI plan:** runs only after every
U task for the shell, Pipeline, Schedule Creator and Tar Packager views and
scenarios has landed.
Project: gpui-migration   Depends on: M5, M6, M7b and their U tasks (landed)

## Goal

Regenerate every image of the user manual from the GPUI app through the
Rust screenshot harness, update the manual's wording where the app changed,
and delete the Node screenshot pipeline.

## Context

FileManager is moving from Tauri + React to GPUI. The manual is
`docs/manual/user-manual.md`; its 30 images are `docs/manual/images/*.png`,
made today by `docs/manual/screenshots/shoot.js`, `lib.js` and `mock.js`
(Playwright, Chromium, a mocked backend) on Linux.

What exists now:

- The harness: `apps/filemanager/src/shoot/` and
  `src/bin/filemanager-shoot.rs` (feature `shoot`, dev profile):
  `filemanager-shoot --out <dir> [--only <names>] [--prefix <p>] [--dark]`;
  `shoot::scenarios()` returns every registered scenario. The tool
  scenarios are in `apps/filemanager/src/shoot/scenarios/` (written by the
  UI tasks), one per manual image, named like the PNG stem.
- CI (windows job) runs the harness twice and uploads `shots/` as the
  `screenshots` artifact; a difference between runs is reported, never
  failed; a scenario that cannot run fails the job. The human decided CI
  **never fails on image differences**.
- `docs/manual/screenshots/README.md` has a "GPUI harness" section.
- The harness refuses a display scale other than 100%; screenshots are taken
  on Windows.

### What to do

1. **Registry test.** Add `shoot::tests::every_manual_image_has_a_scenario`:
   read `docs/manual/user-manual.md` (via `include_str!` with a path from
   `CARGO_MANIFEST_DIR`), collect every `images/<name>.png` reference, and
   assert each `<name>` is a registered scenario; also assert every image
   file in `docs/manual/images/` is referenced by the manual (no orphans).
   Missing scenarios are a gap in a UI task: report them rather than writing
   scenarios yourself.
2. **Regenerate.** Run the harness on Windows at 100% scale (CI artifact or
   a local run) and replace every PNG in `docs/manual/images/` with the
   harness output. Commit them. State in the PR which run produced them (CI
   run URL or machine).
3. **Manual text.** Read `user-manual.md` against the GPUI app and fix
   wording that no longer holds: anything about a browser or webview, page
   reload, the text-size or screen-reader behaviour of the old UI, the Tar Packager table
   stacking rows in narrow windows (it now scrolls sideways), and the notice
   shown when dropped names are not valid Unicode (add one sentence where
   drop is described: such items cannot be dropped, FileManager says so;
   rename them or use Browse, which handles them). File dialogs are the
   Windows file picker with filters, as before. Do not redescribe unchanged
   flows.
4. **Delete** `docs/manual/screenshots/{shoot.js,lib.js,mock.js}`; the README
   keeps only the GPUI harness section (command, Windows at 100%, output
   folder, the CI artifact, that scenarios live in
   `apps/filemanager/src/shoot/scenarios/`).
5. **Side-by-side set for the human.** Add a CI step (windows job, after the
   harness) that builds `shots/side-by-side/` with, per image, the committed
   old PNG (from the commit before this milestone, via `git show`) next to
   the new one, using the `image` crate in a small `shoot` subcommand
   (`filemanager-shoot --compare <old-dir> <new-dir> <out-dir>`), and uploads
   it. The human reviews it before this milestone is approved as done.
6. **Ledger.** Every remaining `open` row of `docs/gpui-test-ledger.md` that
   belongs to a UI task is now settled by it; check that none of the three
   tools' rows is `open`, and list any that are in the PR as a blocker.

**Warning text (note from the M7a re-review, 2026-11-22).** Today's mock
`gateway-svc` warning names an entry that is not in the list. The
regenerated images reproduce that, because the fixture copies the mock. If
the manual's text describes that warning, choose a self-consistent warning
for the fixture, and the mock while it exists. That is a fixture change in
`apps/filemanager/src/tools/tarpack/fixtures.rs` (yours). Record it in the
PR so the side-by-side review expects the difference.

**Two small code nits, carried over from M7b's re-review** (the next
backend run that touches the Tar Packager does them, with a test where a
behaviour changes):

- In `apps/filemanager/src/tools/tarpack/model.rs`, `apply`: split the
  `take()` side effect from the focus-hint condition (take first, then
  test).
- In `apps/filemanager/src/platform/win_shell.rs`: end the fallback detail
  "Windows error <n>" with a full stop.

### Rules that bind this task

- Screenshots use fixture backends and the fake platform only; no real
  files.
- The release exe never contains the harness or GPUI's `test-support`
  (the `--compare` subcommand lives in `filemanager-shoot`).
- No visual changes: if an image looks wrong, report it to the UI task's
  owner; do not edit views or scenarios.
- fmt clean; clippy pedantic `-D warnings` (also with `--features shoot`).

## Files

Edit: `docs/manual/user-manual.md`, `docs/manual/images/*.png`,
`docs/manual/screenshots/README.md`, `apps/filemanager/src/shoot/{mod.rs,…}`,
`apps/filemanager/src/bin/filemanager-shoot.rs`, `.github/workflows/ci.yml`,
`CLAUDE.md` (the `docs/manual/` layout line: images regenerated by the Rust
harness).

Delete: `docs/manual/screenshots/{shoot.js,lib.js,mock.js}`.

Do not touch: `apps/filemanager/src/shoot/scenarios/`, views, theme,
`apps/desktop/` (removed in the next milestone), `docs/plans/`.

## Acceptance criteria

- `every_manual_image_has_a_scenario` passes; there are no orphan images.
- Every PNG in `docs/manual/images/` is the harness output at the scenario's
  size or clip (1366×800 for Tar Packager images, 1000×700 for the rest,
  or the bounds of the scenario's `clip` element for the cropped images).
- No Node file remains under `docs/manual/`.
- The manual mentions no browser, webview, reload of the page, or row
  stacking, and describes the non-Unicode name notice for drop.
- The `side-by-side` artifact exists on the PR's CI run; the human has
  reviewed it (recorded in the PR).
- CI is green on both jobs.

## Tests proving completion

- `shoot::tests::every_manual_image_has_a_scenario` and
  `shoot::tests::no_orphan_manual_images`
  (`cargo test -p filemanager-gpui --features shoot`).
- `shoot::tests::compare_writes_side_by_side` (temp dir, two tiny PNGs).
- The CI harness runs.

## Out of scope

- The Database Manager's manual chapter (its own docs milestone).
- Removing Tauri, React and npm (next milestone).

## Risks

- Every image changes once (DirectWrite and Segoe UI instead of Chromium
  and Inter); text widths differ. That is accepted; colours and layout are
  the comparison.
- A local run at a non-100% display scale is refused; use the CI artifact.
