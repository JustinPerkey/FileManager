# U3b — Rich text with mono paths in banners and confirmations; a shared Cancel label

Status: approved as a plan (2026-11-23). **Items 1–3 run only if the ui-designer chooses rich text** at U6's or U8's critique. Items 4 and 5 (the shared Cancel default, and `Disclosure::track_focus`) run either way, with U8 or before it.
Project: gpui-migration   Depends on: U3 (landed), U5 and U7 (landed)

## Goal

Let a banner's message and details, and a confirmation's body, mix plain and
mono runs, so paths render in mono as today: Schedule's backup line and its
confirmation paths, and the Pipeline confirmation's folder. Also give the
confirm dialog a shared default "Cancel" label, so no view defines its own.

## Context

Shared widgets (U2, U3) are in `apps/filemanager/src/widgets/`:

- `banner.rs`: `Banner::new(Tone, message)`, `.details(text)`,
  `.action(..)`, `.id(..)`;
- `dialog.rs`: `ConfirmSpec { title, body, extra, cancel_label, confirm, busy, wide }`
  and `sync_confirm`.

Both take plain text today. In the React app these places render paths in
mono:

- the Schedule result's backup line;
- the Schedule confirmation's two paths;
- the Pipeline confirmation's folder.

U5 and U7 render them plain. The U5 and U7 reviews left the choice to the
critique: rich text (this plan), or the `extra` line rendered mono (no
widget change).

The Schedule view (`tools/schedule/view/`) defines a private `CANCEL`
constant, because `tools/schedule/text.rs` has no cancel copy.

Copy comes from each tool's `text.rs`, and views never write copy. A rich
text value carries runs: the copy decides which part is a path.

### What to build

1. **`widgets/rich.rs`:** `pub enum Run { Text(SharedString), Mono(SharedString) }`
   and `pub struct Rich(Vec<Run>)`, with
   `From<&str>`/`From<SharedString>` (one plain run), `Rich::text(..)` and
   `Rich::mono(..)` builders, and a render that uses the U2 typography
   helpers (`body`, `mono`) in one wrapping line.
2. **Banner and dialog take `impl Into<Rich>`:**
   - `Banner::new(tone, message)` and `.details(..)`;
   - `ConfirmSpec.body` and `ConfirmSpec.extra`.

   Existing plain-string call sites keep compiling through `From<&str>`.
3. **Use it at the three places.**
   - The Pipeline confirmation, with the folder mono.
   - The Schedule confirmation, with both paths mono.
   - The Schedule result, with the backup path mono.

   Build each `Rich` in the view from the model's copy pieces: the sentence
   from `text.rs`, the path as a mono run. If a tool's `text.rs` only offers
   the whole sentence with the path baked in, report it. The backend
   implementer then splits it into a template and a path.
4. **Shared Cancel default:** `ConfirmSpec::cancel_label` defaults to the
   app copy `crate::app::text::CANCEL` ("Cancel"), which is added there if
   missing. Remove Schedule's private `CANCEL`.

5. **`Disclosure` tracks focus (from the dbmanager U2 review, 2026-11-27).**
   Give `widgets::disclosure::Disclosure` a `track_focus(&FocusHandle)`
   builder, so a caller can focus the toggle and keep keyboard focus on it.
   Switch the Database Manager's hand-built problems toggle
   (`tools/dbmanager/view/`) to `Disclosure` with it.
   Test: `disclosure_track_focus_keeps_focus_on_toggle`.

### Rules that bind this task

- No colour literal outside `theme`. Copy comes from `text.rs` /
  `app::text`. Every interactive element stays keyboard reachable and
  labelled.
- fmt (linux job), clippy pedantic `-D warnings`, `cargo test --workspace`.

## Files

Create: `apps/filemanager/src/widgets/rich.rs`.

Edit:

- `apps/filemanager/src/widgets/{mod.rs,banner.rs,dialog.rs}` and their
  tests;
- `apps/filemanager/src/app/text.rs` (only `CANCEL`);
- `apps/filemanager/src/tools/{pipeline,schedule}/view/`.

Do not touch: models, the tools' `text.rs` (report a needed split),
`docs/plans/`.

## Skill

`impeccable` `critique` on the regenerated Pipeline and Schedule
confirmation and result PNGs (light and `dark-`).

## Acceptance criteria

- The three places render their paths in mono.
- Plain-string callers are unchanged.
- No view defines its own Cancel label.

## Tests proving completion

- `widgets::rich::tests::runs_render_in_order`,
  `widgets::banner::tests::rich_message_and_details`,
  `widgets::dialog::tests::rich_body_and_default_cancel`.
- `pipeline` and `schedule` view tests: the confirmation's mono run holds the
  path (a tag on the mono run, checked through `probe::bounds`, or a
  structural check in `VisualTestContext`).

## States covered

Confirmation open, result with a backup line.

## Out of scope

- The Tar Packager and Database Manager uses: they adopt `Rich` in their own
  tasks if their critique asks for it.
