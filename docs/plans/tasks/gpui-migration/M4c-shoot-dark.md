# M4c — Make the screenshot harness's `--dark` work

Status: approved (2026-11-22; from the U1 review)
Project: gpui-migration   Depends on: U1 (landed, with `theme::set_override`)

## Goal

`filemanager-shoot --dark` renders every scenario in the dark theme, so the
Pipeline, Schedule Creator and Tar Packager scenario tasks can produce their
dark PNGs.

## Context

The screenshot harness is in `apps/filemanager`, feature `shoot`:

- `src/shoot/mod.rs`: `run_scenario`, which builds the app with
  `crate::init_app(cx, setup)` and then `crate::open_main_window(cx, size)`;
- `src/shoot/cli.rs`: argument parsing and the child runner;
- `src/bin/filemanager-shoot.rs`: the binary.

`--dark` is parsed, but it only prints
`note: --dark has no effect yet (the theme override is not available)` (the
bin, around line 50). The `shoot/mod.rs` doc (around line 32) and
`docs/manual/screenshots/README.md` (around line 35, "--dark (no effect until
the theme task)") say the same.

The theme task (U1) has since added
`fm_app::theme::set_override(Option<ThemeMode>, cx)`. `Some(ThemeMode::Dark)`
forces dark whatever the system says; `None` follows the system.

### What to build

- **In `run_scenario`,** when dark is requested, call
  `theme::set_override(Some(ThemeMode::Dark), cx)` after `init_app` and
  **before** `open_main_window`, so the first frame is already dark. Carry
  the flag from the CLI into each child scenario process (it is per run).
- **Remove every "no effect" note:**
  - the `eprintln!` in the bin;
  - the sentence in the `shoot/mod.rs` doc, replaced by one saying what
    `--dark` does;
  - the README's option line, now `--dark   render in the dark theme`.
- **PNG names:** keep whatever M4 defined for dark output (a prefix or
  suffix). If none, `--dark` writes `<prefix><name>.png` like the light run,
  and the caller passes `--prefix dark-` or a separate `--out` folder.
  Document which in the README.

### Rules that bind this task

- `test-support` and the harness never reach the release exe.
- Do not touch scenarios (`src/shoot/scenarios/`) or the theme module.
- fmt (linux job), clippy pedantic with and without `--features shoot`, and
  `cargo test --workspace` pass.

## Files

Edit:

- `apps/filemanager/src/shoot/{mod.rs,cli.rs,tests.rs}`;
- `apps/filemanager/src/bin/filemanager-shoot.rs`;
- `docs/manual/screenshots/README.md`.

## Acceptance criteria

- With `--dark`, the window's theme is dark before the first frame, for
  every scenario.
- No "no effect" text remains in the bin, the module doc or the README.

## Tests proving completion

- `shoot::tests::dark_flag_sets_override_before_first_frame`
  (`#[gpui::test]`: run the sample scenario's setup with dark requested; the
  active theme mode is dark when the window opens).
- `shoot::cli::tests::dark_flag_reaches_child_args`.
- On Windows CI, one dark run of the sample scenario writes a PNG.

## Out of scope

- Dark scenarios themselves (U6, U8, U12), and the theme.
