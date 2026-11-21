# M3a2 — Developer-mode chord independent of focus; M3a follow-ups (CI output, CLAUDE.md, lib.rs)

Status: approved (2026-11-21; the reviewer's condition on M3a)
Project: gpui-migration   Depends on: M3a (landed). Runs before M3b.

## Goal

Make Ctrl+Alt+Shift+D toggle developer mode whatever has focus, as today's
app does, and close M3a's review follow-ups: stale `CLAUDE.md` lines, a
wrong comment in `lib.rs`, and two Linux CI checks that print nothing.

## Context

`apps/filemanager` (package `filemanager-gpui`, lib `fm_app`) was built by
M3a. Developer mode is `src/app/dev_mode.rs`: a global `DevMode(bool)`
(`init`, `is_on`, `toggle`, observers via `cx.observe_global::<DevMode>`),
`DevMode::handle_key(&KeyDownEvent, cx)` (toggles on a fresh chord, ignores
`is_held` repeats), called from an `on_key_down` listener on the `Shell`
(`src/app/shell.rs`), which tracks its own focus handle and focuses itself
when the window opens.

**The defect (found in review, reproduced).** GPUI delivers element key
listeners only along the focused path, so the chord is lost when:

- the window has no focused element: gpui-component's `Root` blurs it when it
  cannot restore focus after a dialog;
- the focused element stops rendering (for example after a tool switch);
- focus is inside a dialog, sheet or notification layer, which `Root`
  renders as **siblings** of the Shell, not inside it.

Blur the window, send the chord: developer mode stays off. Today's
`apps/desktop/src/app/devMode.tsx` listens on `window`, so it works whatever
has focus, and the chord must keep doing so: it toggles on a fresh press,
an auto-repeated (held) chord does not toggle again, and it is absent from
every shortcuts list.

### What to build

**1. App-level chord handling.** Replace the Shell's `on_key_down` path with
an app-level keystroke interceptor, installed once in `DevMode::init`:
`cx.intercept_keystrokes(|event, window, cx| …)` (read its exact signature
and the `KeystrokeEvent` fields in the pinned `gpui-pre` 0.3.12 source; keep
the returned `Subscription` alive in the global or `detach()` it). It sees
every key-down the window dispatches, before the focus path, so no focus is
needed.

**A held chord toggles once.** Use the best signal the pinned GPUI gives, in
this order, and say in the module doc and the PR which one you used:

1. if the intercepted event (or `window` while dispatching it) exposes
   whether the key is an auto-repeat (`is_held`), toggle only when it is
   false;
2. otherwise a **latch**: toggle on a chord key-down only while the latch
   is clear, then set it; clear it when the chord is released, i.e. on a
   key-up of `d` or a modifiers change that drops Ctrl, Alt or Shift,
   observed through a window-level hook that does not depend on focus (for
   example a listener the Shell registers each frame with
   `window.on_key_event` / `window.on_modifiers_changed`, if those fire
   outside the focus path in the pinned source; check). If no such hook
   exists, clear the latch on the next non-chord keystroke the interceptor
   sees or when the window loses activation, and document that a chord held
   across a release the app could not see needs one extra press.

The chord match stays `is_chord` (Ctrl+Alt+Shift, not platform or
function, key `d`, case-insensitive). Remove the Shell's key listener and
its self-focusing if nothing else needs them; keep `DevMode::handle_key` only
if it is still the single place that decides "toggle or not" (the
interceptor calls it).

**2. Tests** (`src/app/dev_mode.rs`, `#[gpui::test]`):

- `chord_works_without_focus` — **restored to its original meaning:** a
  fresh window, then explicitly blur it (`window.blur()`), assert nothing
  is focused, send the chord, developer mode is on.
- `chord_works_after_focus_is_lost` — focus an element, remove it from the
  tree (or switch tools), send the chord: it toggles.
- `chord_works_with_focus_in_a_dialog` — open a gpui-component dialog
  (`window.open_dialog`) containing an `Input`, focus the input, send the
  chord: it toggles, and the input received no `d`.
- `held_chord_toggles_once` — a fresh chord key-down then repeated
  key-downs with `is_held = true` (and, if you used the latch, no
  intervening release): one toggle; after the release (key-up or modifiers
  change, simulated with `simulate_event`), a new chord toggles again.
- Keep `off_at_launch`, `chord_toggles` (incl. other chords do nothing),
  `observers_are_notified`.

**3. Ledger.** In `docs/gpui-test-ledger.md`, the two `app/devMode.test.tsx`
rows owned by M3a were set back to `open` (2026-11-21) until this lands.
Mark them `done` again with the test names above.

**4. `apps/filemanager/src/lib.rs`.** Its comment says the workspace lints
reject an unused dependency; they do not (pedantic clippy has no such lint,
and rustc's `unused_crate_dependencies` is not enabled). Remove the
`use fm_core as _;` … `use fm_tarpack as _;` lines and the comment. The
`fm-*` dependencies stay in `Cargo.toml` (later milestones use them, and the
dumpbin check wants them linked once a tool uses them).

**5. CI output (`.github/workflows/ci.yml`, linux job).**

- The five-crate dependency check: print each `cargo tree` result line
  (`echo "$c / $dep: not a dependency"`) so a green run shows what it
  checked.
- The test-support check stays in the **linux** job, cross-target (it only
  reads metadata; no Windows build is needed):
  `cargo tree --locked -p filemanager-gpui -e features,normal,build --target x86_64-pc-windows-msvc -i gpui-pre`.
  Print its output. Add a **positive control**: fail if the output does not
  contain `gpui-pre feature "default"` (so a command that silently matches
  nothing cannot pass), then fail if it contains `test-support`.

**6. `CLAUDE.md`, stale lines from M3a.**

- The root `Cargo.toml` line: members `crates/*`, `devtools/*`,
  `apps/filemanager`, `apps/desktop/src-tauri`; `[workspace.dependencies]`
  pins `gpui` (`gpui-pre` =0.3.12), `gpui_platform` (`gpui-pre-platform`
  =0.3.12, no default features), `gpui-component` =0.10.0,
  `gpui-kit-assets` =0.10.0, `async-channel`, `notify-debouncer-mini`,
  `ts-rs`, `tempfile` (whatever the file actually pins: copy it).
- The `ci.yml` line: linux job runs fmt, clippy, tests, the five-crate
  dependency check and the test-support check; windows job
  (`windows-2025`) runs clippy, tests, both release builds, dumpbin on both
  exes, the GPUI self-check, and uploads `FileManager-<version>-x64.exe` and
  `FileManager-gpui-<version>-x64.exe`.
- In the Commands notes, "runs `dumpbin /dependents` on the exe" becomes
  "on both exes (`filemanager.exe`, `filemanager-gpui.exe`)".
- The Linux scoping note (`cargo can be scoped to -p …`) adds
  `-p filemanager-gpui` (it builds headless on Linux without the webkit
  packages).
- The `fonts.rs` lines say it picks the system mono font (if not already).

### Rules that bind this task

- Developer mode stays app-wide, off at launch, absent from shortcut lists.
- No behaviour change elsewhere; the Shell's public API is unchanged.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.
- Tests touch no files.

## Files

Edit: `apps/filemanager/src/app/{dev_mode.rs,shell.rs}`,
`apps/filemanager/src/lib.rs`, `.github/workflows/ci.yml`, `CLAUDE.md`,
`docs/gpui-test-ledger.md`.

Do not touch: `crates/`, `apps/desktop/`, `docs/plans/`.

## Acceptance criteria

- The four focus tests above pass on both CI jobs; `chord_works_without_focus`
  blurs the window explicitly.
- No `on_key_down` chord listener remains on the Shell (or, if one remains
  for another reason, it no longer handles the chord).
- The linux CI log shows each dependency-check line and the `cargo tree`
  output; the positive control is in place.
- `lib.rs` has no `use fm_* as _;` lines.
- `CLAUDE.md` matches `Cargo.toml` and `ci.yml`.
- fmt (linux job), clippy pedantic and `cargo test --workspace` (both jobs)
  pass; dumpbin and self-check still pass.

## Tests proving completion

`cargo test -p filemanager-gpui dev_mode`: `off_at_launch`, `chord_toggles`,
`held_chord_toggles_once`, `chord_works_without_focus`,
`chord_works_after_focus_is_lost`, `chord_works_with_focus_in_a_dialog`,
`observers_are_notified`.

## Out of scope

- `Platform`, jobs, drop routing, close guard (M3b).

## Risks

- `intercept_keystrokes` runs before key bindings: return without consuming
  anything except the chord, so no other shortcut is affected. If the pinned
  API cannot stop propagation, the chord still reaches focused elements; the
  dialog test checks the input receives no `d`, and if that cannot be
  prevented, say so in the PR.
