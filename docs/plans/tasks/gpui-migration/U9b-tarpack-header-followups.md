# U9b — Tar Packager follow-ups from the U9 review: the errors toggle chevron, Edit in editor in the errors region, open ledger rows

Status: approved (2026-11-23; from the U9 review, which approved U9)
Project: gpui-migration   Depends on: U9 (landed). Run it after U10 and before U12 (not at the same time as U10).

## Goal

Restore two details of today's errors region, flipping the "Show errors" /
"Hide errors" chevron and the developer-mode **Edit in editor**, and settle
the ledger rows U9 left open.

## Context

U9 built the Tar Packager's frame, header, banners, errors region and drop
result in `apps/filemanager/src/tools/tarpack/view/`, over `TarpackModel`
(`src/tools/tarpack/model.rs`: `errors_expanded`, `set_errors_expanded`,
`show_errors`, `can_edit(cx)`, `edit(cx)`, `drop_state`, …). Shared widgets
are in `apps/filemanager/src/widgets/`. `FmButton` (`button.rs`) has
`.icon(IconName)` for a leading icon and `.icon_flip()`, which today rotates
only the **leading** icon.

Today's React errors region (`apps/desktop/src/tools/tarpack/ManifestErrors.tsx`):

- the toggle reads "Show errors" or "Hide errors", with a **trailing**
  chevron that flips with the state;
- in developer mode it shows an **Edit in editor** button beside the toggle
  (`{devMode && <Button onClick={onEdit}>Edit in editor</Button>}`).

### What to build

1. **The trailing chevron flips.**
   - Give `FmButton` a trailing icon, `.trailing_icon(IconName)`, if it
     lacks one.
   - Make `.icon_flip()` apply to whichever icon the button has. If it has
     both, add `.flip_trailing()`.
   - The errors toggle shows `chevron-down` trailing, flipped while expanded.
   - Test: `errors_toggle_chevron_flips_with_state`.
   - Check the build bar's Show errors too: U11 uses the same widget, and the
     result must match.
2. **Edit in editor in the errors region, in developer mode.** Today's
   behaviour, restored:
   - beside the toggle, a secondary **Edit in editor** button, shown only
     when `can_edit(cx)` (developer mode and a manifest), calling `edit(cx)`;
   - tag `tarpack-errors-edit`;
   - the copy comes from `text.rs`.
   - Test: `errors_region_edit_follows_dev_mode`.
3. **Ledger.** In `docs/gpui-test-ledger.md`, settle every row now owned by
   `U9b`: the `DropResult`, `ManifestErrors`, `ManifestHeader`,
   `DiagnosticList`, `ShortcutsHelp`, `TarpackView.errors` and
   `TarpackView.states` rows, and the devMode row "the shortcuts list names
   Ctrl+E only in developer mode". For each, either:
   - name the existing U9 test that covers it;
   - add a named view test; or
   - mark it `dropped: <reason>` (axe, `aria-*`, DOM structure).

   In particular:
   - **"Full paths: collapsed, expands, lists every path past the cap"** and
     **"Full paths is absent for matched-only outcomes":** close them against
     `drop_result_lines_and_dismiss` only if that test asserts each point.
     Otherwise add `drop_result_full_paths_disclosure`.
   - **"empty outcome: text and icon are muted, no disclosure":** add
     `drop_result_empty_outcome_is_muted` if no test covers it.

### Rules that bind this task

- Views call only the model; copy from `text.rs`. Every interactive element
  is keyboard reachable and labelled. Tags are the last call of a builder.
- No colour literal outside `theme`.
- fmt (linux job), clippy pedantic `-D warnings`, `cargo test --workspace`.

## Files

Edit:

- `apps/filemanager/src/widgets/button.rs` and its tests;
- `apps/filemanager/src/tools/tarpack/view/` (the errors region, tests);
- `docs/gpui-test-ledger.md`.

Do not touch: models, text (report missing copy), the entry table (U10),
the build bar (U11), `docs/plans/`.

## Skill

None (restoring today's look). Do not run `impeccable`.

## Acceptance criteria

- The named tests pass. No `U9b` row stays `open`.

## Tests proving completion

- `widgets::button::tests::trailing_icon_flips`.
- `errors_toggle_chevron_flips_with_state`, `errors_region_edit_follows_dev_mode`.
- Any ledger tests added in step 3.

## States covered

Errors collapsed and expanded, developer mode on and off.

## Out of scope

- The table (U10) and the build side (U11).
