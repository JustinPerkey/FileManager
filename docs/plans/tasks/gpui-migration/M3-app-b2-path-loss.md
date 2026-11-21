# M3b2 — Path-loss notice only for drags, the second skip message, and drops during a modal

Status: approved (2026-11-21; the planner's decisions after the M3b review, and the human's decision on drops during a modal)
Project: gpui-migration   Depends on: M3b (landed)

## Goal

Make the "dropped items could not be read" notice appear for drops and
never for pastes, report the second way GPUI silently skips a dropped item,
and refuse file drops while a modal dialog is open.

## Context

M3b built `apps/filemanager/src/platform/path_loss.rs` and the Shell's drop
and notice handling (`src/app/shell.rs`: `ShellNotice::DroppedNamesUnreadable`,
`Shell::notice`, `Shell::dismiss_notice`, `Shell::drag_over`, the
`on_drop::<ExternalPaths>` path, `ToolHooks::drop_target` with
`DropTarget::{None, Enabled { label }, Disabled { reason }}`). Read them
first. Today:

- The logger matches GPUI's Error record from target
  `gpui_windows::clipboard`, text `"dragged file name is not UTF-16: {err}"`
  (gpui-pre-windows `clipboard.rs` about line 145), by the prefix
  `PATH_LOSS_MESSAGE = "dragged file name is not UTF-16"` followed by the end
  of the message or `':'`. A Windows-only test pins the prefix against the
  `gpui-pre-windows` source.
- The Shell coalesces signals within one drag, **or within 1 s if no drag
  events come**, into one notice.

Two problems found in review:

1. **False notice on paste.** GPUI's clipboard `read_files`
   (`clipboard.rs` about line 271) calls the same `with_file_names`, so
   pasting after copying a file whose name has a lone surrogate in Explorer
   logs the same text, and the no-drag fallback shows "Some dropped items
   could not be read" though nothing was dropped. On Windows an HDROP drag
   always produces `FileDropEvent::Entered`, even when every name fails, so a
   drop always has drag events around it.
2. **A second silent skip.** When `DragQueryFileW` returns 0, GPUI logs
   `"unable to read file name of dragged file"` and skips the item. The
   invariant says a dropped item is never skipped silently.

And the human decided: **while a modal layer is open** (a gpui-component
dialog over the window), a file drop is refused, the overlay shows its
Disabled state, and the drag ends cleanly.

### What to build

1. **Count signals only around a drag.** The Shell keeps the drag window:
   open from `Entered` until `Submit` or `Exited`, plus 1 s after either.
   A path-loss signal received inside that window counts (coalesced into
   one notice per drag, as now); a signal outside it is ignored. Remove the
   no-drag 1 s fallback. Timing uses the GPUI executor's clock
   (`cx.background_executor().now()` or a timer), so tests can advance it.
2. **The second message.** `path_loss.rs` also recognises
   `UNREADABLE_NAME_MESSAGE = "unable to read file name of dragged file"`
   (Error level, target starting with `gpui`; same prefix rule: followed by
   the end of the message or `':'`) and sends a distinct signal. The Shell
   shows a second notice kind, `ShellNotice::DroppedItemsUnreadable`, with
   the copy in `src/app/text.rs`: "Some dropped items could not be read.
   Drop them again, or use Browse." If both kinds arrive in one drag, show
   the non-Unicode one (it tells the user what to fix). The pin test checks
   both literals against the `gpui-pre-windows` source.
3. **Drops during a modal.** While `window.has_active_dialog(cx)` is true
   (check the name in the pinned gpui-component), `Shell::drag_over()`
   returns `Disabled { reason }` with the Shell's reason, `src/app/text.rs`
   `DROP_BLOCKED_BY_DIALOG = "Close the open dialog first."`, whatever the
   current tool reports; a drop calls nothing; `Submit`/`Exited` clear the
   drag state so the overlay goes away.

### Rules that bind this task

- A dropped item is never skipped silently; a paste never shows a drop
  notice.
- Detection stays behind `path_loss.rs`; models are not touched.
- Tests touch no files; fmt clean; clippy pedantic `-D warnings`; scoped
  `#[expect]` only (the windows job lints `#[cfg(windows)]` code).

## Files

Edit: `apps/filemanager/src/platform/path_loss.rs`,
`apps/filemanager/src/app/{shell.rs,text.rs}`, and their tests.

Do not touch: tools, `crates/`, `apps/desktop/`, `docs/plans/`.

## Acceptance criteria

- No notice is shown for a path-loss record that arrives with no drag in
  progress and none ended in the last second.
- A record during a drag, or within 1 s after it ends, shows one notice per
  drag.
- The unreadable-name record raises `DroppedItemsUnreadable`.
- With a dialog open, the overlay state is Disabled with the dialog reason
  and a drop reaches no tool.
- fmt (linux job), clippy pedantic and `cargo test --workspace` (both jobs)
  pass.

## Tests proving completion

- `platform::path_loss::tests::matches_prefix_with_detail`,
  `matches_unreadable_name_message`, `other_records_do_not_signal`.
- `platform::path_loss::tests::gpui_message_texts_are_pinned`
  (`#[cfg(windows)]`, both literals).
- `app::shell::tests::path_loss_outside_a_drag_is_ignored` (the paste case),
  `path_loss_during_drag_shows_one_notice`,
  `path_loss_within_1s_after_drag_counts`,
  `path_loss_after_1s_is_ignored`,
  `unreadable_name_shows_its_own_notice`,
  `non_unicode_notice_wins_when_both_arrive`.
- `app::shell::tests::drop_refused_while_dialog_open`
  (`window.open_dialog`, then `Entered`: `drag_over()` is Disabled with the
  dialog reason; `Submit`: the tool's `dropped` is not called and
  `drag_over()` is `None`).

## Out of scope

- The look of the notices and the overlay (UI task U4).

## Risks

- Both detections depend on GPUI's log texts; the pin test fails on an
  upgrade that changes them.
- A drag whose `Exited` never arrives would leave the window open; `Submit`
  or `Exited` always ends it on Windows, and the 1 s tail is bounded.
