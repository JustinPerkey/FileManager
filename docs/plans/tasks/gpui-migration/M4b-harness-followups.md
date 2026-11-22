# M4b — Screenshot harness follow-ups: fresh tags, binary tests, timeouts, CI controls

Status: approved (2026-11-22; the reviewer's follow-ups on M4)
Project: gpui-migration   Depends on: M4 (landed)

## Goal

Close M4's review follow-ups. The harness may resolve only tags drawn in the
last frame. The `filemanager-shoot` binary gets tests and a child timeout.
The CI steps lose a stale comment and gain a negative control. The docs say
how tags and scenarios actually work.

## Context

M4 built the screenshot harness in `apps/filemanager`, behind feature
`shoot`:

- **`src/app/probe.rs`:** `.tag("<name>")` on an element records its bounds
  into a thread-local `HashMap<SharedString, Bounds<Pixels>>` (test and
  `shoot` builds only; a pass-through otherwise). It has `bounds(name)` and
  `reset()`, which the harness calls once per scenario. A tag keeps the bounds
  of its latest paint, so an element that stopped being drawn still resolves
  to its old bounds until `reset`.
- **`src/shoot/mod.rs`:**
  - `Scenario { name, size, setup, steps, clip }`, where `clip` and
    `Step::Click` are resolved **only by tag names**: GPUI's `debug_bounds`
    is readable only inside `VisualTestContext`;
  - `run_scenario(scenario, out_dir, prefix)`, which builds the app with
    `crate::init_app(cx, setup)` and `crate::open_main_window(cx, size)`
    rather than `run_with`;
  - `WAIT_LIMIT` = 30 s for a step's wait.
- **`src/shoot/steps.rs`:** the step player. `Step::WaitIdle` is a short
  fixed settle.
- **`src/bin/filemanager-shoot.rs`:**
  - `parse(args) -> Result<Args, String>` reads `--out`, `--only`,
    `--prefix` and `--dark`;
  - it runs each scenario in a child process
    (`Command::new(current_exe)`);
  - it prints `ok <name>` / `FAIL <name>: <reason>` and exits non-zero if
    any failed.
- **`.github/workflows/ci.yml`:**
  - windows job: a "Screenshots" step that builds the harness in the dev
    profile (its comment says "GPUI records debug selectors only in debug
    builds") and runs it twice;
  - linux job: the release-graph check
    `cargo tree --locked -p filemanager-gpui -e features,normal,build --target x86_64-pc-windows-msvc -i gpui-pre`,
    which fails on `test-support` and has a positive control for
    `gpui-pre feature "default"`.

**Decided after M4:** the harness stays in the **dev profile**, because
that build reuses the test build's artifacts. Debug selectors are no longer
the reason.

### What to build

1. **Only tags drawn in the last frame resolve.** Frame-stamp each entry
   (record a frame counter the probe advances once per window draw) and make
   `bounds(name)` return `None` for an entry not drawn in the latest frame.
   Clearing the map at the start of each draw is the alternative; use it if
   GPUI gives a reliable per-draw hook. Either way, a tag whose element left
   the tree must not resolve, and the harness then reports "tag not found",
   not a click at stale coordinates. `reset()` stays for scenario isolation.
2. **`probe.rs` doc:**
   - `.tag("<name>")` must be the **last** call in a builder chain
     (outermost), so the recorded bounds are the element's final bounds;
   - what "latest frame" means after step 1;
   - debug selectors are for `VisualTestContext` tests only.
3. **`src/shoot/mod.rs` doc:**
   - `run_scenario` uses `init_app` and `open_main_window` rather than
     `run_with`, and why (the per-scenario window size and setup);
   - `Step::WaitIdle` is a fixed settle that does not await `Worker`, thread
     jobs or async loads. Fixtures must answer synchronously, or the
     scenario adds `Step::Wait(ms)`.
4. **Binary tests** (`src/bin/filemanager-shoot.rs` `#[cfg(test)]`, or move
   `parse` into the lib's `shoot` module so it is testable):
   - `parse_out_is_required`, `parse_only_splits_on_commas`,
     `parse_prefix_and_dark`, `parse_rejects_unknown_flag`,
     `parse_rejects_missing_value`.
5. **Child timeout:** kill a child scenario that runs longer than **60 s**,
   report `FAIL <name>: timed out after 60 s`, and go on to the next
   scenario. Count it as a failure for the exit code.
6. **CI, windows job "Screenshots" step:**
   - drop the stale comment about debug selectors; say instead that the dev
     profile reuses the test build;
   - add a check that runs `target\debug\filemanager-shoot.exe --out shots\none --only no-such`
     and fails the job unless that **exits non-zero**.
7. **CI, linux job, release-graph check: add a negative control.** Run the
   same `cargo tree` with `--features shoot` and fail unless that output
   **does** contain `test-support`, so a check that can never see the
   feature cannot pass. Print both outputs.

### Rules that bind this task

- The release exe never contains GPUI's `test-support` or the harness.
  `probe.rs` stays a pass-through outside test and `shoot` builds.
- Screenshots use fixtures and `FakePlatform` only.
- No change to scenarios (`src/shoot/scenarios/`, the ui-implementer's) or
  views.
- fmt (linux job), clippy pedantic with and without `--features shoot`, and
  `cargo test --workspace` pass; scoped `#[expect]` only.

## Files

Edit:

- `apps/filemanager/src/app/probe.rs`;
- `apps/filemanager/src/shoot/{mod.rs,steps.rs,tests.rs}`;
- `apps/filemanager/src/bin/filemanager-shoot.rs`;
- `.github/workflows/ci.yml`;
- `CLAUDE.md`, the `probe.rs` and `shoot/` layout lines, if they change.

Do not touch: `src/shoot/scenarios/`, views, `crates/`, `apps/desktop/`,
`docs/plans/`.

## Acceptance criteria

- A tag whose element was not drawn in the latest frame resolves to `None`.
  A `Click` or `clip` on it fails the scenario with "tag not found".
- The five `parse` tests pass.
- A child that hangs is killed after 60 s and reported. The other scenarios
  still run.
- CI shows the `--only no-such` check exiting non-zero.
- CI shows the release-graph negative control finding `test-support` with
  `--features shoot`, and not finding it without.
- The stale comment is gone; the docs say what is listed above.

## Tests proving completion

- `app::probe::tests::tag_not_drawn_in_latest_frame_does_not_resolve`
  (`#[gpui::test]`: draw a tagged element, then a frame without it, then
  `bounds` is `None`) and `tag_drawn_again_resolves_to_new_bounds`.
- `shoot::tests::click_on_stale_tag_fails_the_scenario`.
- The five `parse_*` tests.
- `shoot::tests::child_timeout_is_reported`. Make the timeout injectable for
  the test (for example a hidden `--child-timeout-ms` flag or a
  `run_children(.., timeout)` function) so the test takes milliseconds, not
  60 s.
- The two CI checks.

## Out of scope

- New scenarios, idle detection (the fixed settle stays; fixtures adapt).

## Risks

- A per-frame hook may not exist in the pinned GPUI. The frame stamp then
  comes from the probe's own paint callbacks: advance the counter when the
  root element paints, before its children.
