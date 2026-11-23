# GPUI test ledger

Every case of the Tauri app's frontend tests (`apps/desktop/src/**/*.test.ts(x)`)
and where it goes in the GPUI app. Columns: `Case` is the test's title (an
`it.each` or `test.each` counts as one case, under its title template);
`Owner` is who ports it (`M3a`, `M3b`, `M5` ... are backend milestones, `U` is
a UI task, `-` means not ported); `New test` names the GPUI test that settles
it, or the reason it is dropped; `Status` is `open`, `done` (ported) or
`dropped`. The cutover milestone refuses to start while any row is `open`.
An owner written `X or U` is a proposal: the milestone that reaches the row
decides which side its test belongs to and updates the row.

Counting rule: one row per match of
`grep -cE "^\s*(it|test)(\.each)?\(" <file>`, so the number of rows in a
file's table equals that count. The file counts sum to 309. A milestone may split a case into a model half
(`done`) and a view half (a further `U` row), so a table can hold more rows than
its heading's count.


## `App.test.tsx` (1)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders the shell with the Pipeline view first | U | `app::view::tests::nav_click_selects_tool` (Pipeline is current before any click; the chrome shows its view) | done |

## `app/AppShell.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders registry entries | M3a | `tools::tests::registry_order_and_labels` | done |
| selecting a tool renders its view | U | `app::view::tests::nav_click_selects_tool` | done |
| shows an empty state with no tools | U | dropped: the registry is never empty (`Shell::with_tools` requires a tool) | dropped |
| has no axe violations | U | dropped: axe is a DOM check; the nav's labels are the buttons' accessibility labels | dropped |

## `app/Banner.test.tsx` (3)

| Case | Owner | New test | Status |
|---|---|---|---|
| %s tone has the right live role and hidden word | U | dropped: the tone word and live role are for a screen reader (out of scope); the tone is the icon and the outline colour, drawn in `widgets::tests::gallery_draws_and_every_control_is_a_tab_stop` | dropped |
| action and dismiss | U | `widgets::tests::banner_dismiss_and_action` (click and Enter on each) | done |
| renders children | U | `widgets::tests::banner_details_toggle` (the message and the details content render; a GPUI banner takes a message and `.details(..)`, not children) | done |

## `app/Button.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| variants | U | `widgets::tests::gallery_draws_and_every_control_is_a_tab_stop` draws all four; the fills and outlines are checked on the Windows screenshots | done |
| forwards ref and disabled | U | disabled half: `widgets::tests::disabled_button_ignores_activation`; ref forwarding dropped: DOM | done |
| busy sets aria-busy and disables | U | `widgets::tests::busy_button_is_inert_and_keeps_label` (aria-busy dropped: screen reader) | done |
| icon renders aria-hidden and no axe violations | U | dropped: aria-hidden and axe are DOM and screen reader checks; `widgets::tests::icon_only_button_has_tooltip` covers the labelling | dropped |

## `app/ConfirmDialog.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| focus starts on Cancel, Escape cancels, focus returns | U | `widgets::tests::confirm_focus_starts_on_cancel_and_escape_cancels`, `widgets::tests::keyboard_opened_dialog_returns_focus_to_opener` | done |
| Replace confirms | U | `widgets::tests::confirm_button_confirms` | done |
| no axe violations when open | U | dropped: axe is a DOM check; the dialog's labels are covered by `widgets::tests::confirm_button_confirms` (named buttons) | dropped |
| aria-labelledby and aria-describedby resolve to the title and body, with unique ids | U | dropped: DOM and ARIA attributes; GPUI has no ids to resolve | dropped |

## `app/DropZone.test.tsx` (6)

| Case | Owner | New test | Status |
|---|---|---|---|
| idle: renders nothing | M3b | `app::shell::tests::drag_over_tracks_enter_and_exit` (`drag_over()` is `None`; the chrome renders nothing from it) | done |
| hover shows the label on a plate; leave hides it | M3b, U | `app::shell::tests::drag_over_tracks_enter_and_exit` (state); the plate's look is U | done (state); the look: `app::view::tests::drop_overlay_shows_label_or_reason` (done) |
| drop hides the overlay and passes exactly the dropped paths | M3b | `app::shell::tests::drop_reaches_current_tool_when_enabled`, `dropped_non_unicode_path_survives_routing` (Windows) | done |
| disabled: overlay shows the reason and a drop is ignored | M3b, U | `app::shell::tests::drop_ignored_when_disabled`, `empty_drop_ignored`; the look: `app::view::tests::drop_overlay_shows_label_or_reason` (the reason on the plate) | done |
| does not steal focus, and unsubscribes on unmount | M3b | dropped: the shell's drop handling is listeners owned by the root element and the window's frame (nothing to subscribe or unsubscribe), and it never touches focus | dropped |
| after a rerender, drops follow the latest enabled and onDrop | M3b | `app::shell::tests::drop_goes_to_the_current_tool_only` (the target is read from the tool at drop time) | done |

## `app/ToolNav.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| marks the active item with aria-current | U | `app::view::tests::current_item_is_marked` (the dot; the button is also `toggled` for accessibility) | done |
| arrow keys move focus and wrap | U | `app::view::tests::nav_keys_move_wrap_and_select`, `only_the_current_item_is_a_tab_stop`, `app::view::sidebar::tests::keys_wrap_and_jump` | done |
| Enter and Space select the focused item | U | `app::view::tests::nav_keys_move_wrap_and_select` | done |
| the current item keeps aria-current and has no thick side border | U | dropped: the marker is covered by `current_item_is_marked`; the missing side stripe is a look (CSS) | dropped |

## `app/devMode.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| Ctrl+Alt+Shift+D toggles developer mode and announces it | M3a | `app::dev_mode::tests::chord_toggles` (announcement strings are constants in `app/text.rs`; the aria-live announcer itself is the UI's decision); focus independence: `chord_works_without_focus`, `chord_works_after_focus_is_lost`, `chord_works_with_focus_in_a_dialog`; held chord: `held_chord_toggles_once` | done |
| other chords leave developer mode off | M3a | `app::dev_mode::tests::chord_toggles` (the other-chord assertions) | done |
| without developer mode, Edit in editor and the extraction command are hidden | M7a | `tools::tarpack::tests::can_edit_needs_dev_mode_and_manifest` | done |
| the shortcuts list names Ctrl+E only in developer mode | U9b |  | open |

## `app/useAnnouncer.test.tsx` (5)

| Case | Owner | New test | Status |
|---|---|---|---|
| the first state is empty and announce is stable | U |  | open |
| announce commits the clear before the frame and the text after it | U |  | open |
| a second announce before the frame replaces the pending text | U |  | open |
| unmounting cancels the frame | U |  | open |
| a repeat from an async continuation is cleared in the DOM before the text returns | U |  | open |

## `lib/pipeline.test.ts` (1)

| Case | Owner | New test | Status |
|---|---|---|---|
| %s invokes the right command | - | IPC wrapper; no IPC in the GPUI app | dropped |

## `lib/schedule.test.ts` (1)

| Case | Owner | New test | Status |
|---|---|---|---|
| %s invokes the right command | - | IPC wrapper; no IPC in the GPUI app | dropped |

## `lib/tarpack.test.ts` (3)

| Case | Owner | New test | Status |
|---|---|---|---|
| %s invokes the right command | - | IPC wrapper; no IPC in the GPUI app | dropped |
| revealOutput passes no arguments | - | IPC wrapper; no IPC in the GPUI app | dropped |
| subscribes to the two events | - | IPC wrapper; no IPC in the GPUI app | dropped |

## `lib/tauri.test.ts` (6)

| Case | Owner | New test | Status |
|---|---|---|---|
| call forwards the command and args | - | IPC wrapper; no IPC in the GPUI app | dropped |
| on hands the payload to the handler | - | IPC wrapper; no IPC in the GPUI app | dropped |
| openFileDialog opens a single file with filters | - | IPC wrapper; no IPC in the GPUI app | dropped |
| openDirectoryDialog opens a single folder | - | IPC wrapper; no IPC in the GPUI app | dropped |
| saveFileDialog passes defaultPath and filters | - | IPC wrapper; no IPC in the GPUI app | dropped |
| onDragDrop yields the type and paths | - | IPC wrapper; no IPC in the GPUI app | dropped |

## `styles/tokens.test.ts` (6)

| Case | Owner | New test | Status |
|---|---|---|---|
| dark hooks define identical values | U | `theme::tests::tokens_match_tokens_css` (both dark blocks of tokens.css, the media query and `[data-theme=dark]`, equal each other and `DARK`) | done |
| ${name}: %s on %s meets 4.5:1 | U | `theme::tests::contrast_pairs_meet_4_5` | done |
| %s defines the overlay shadow | U | `theme::tests::tokens_match_tokens_css` (shadow alpha, both themes) | done |
| %s defines --scrim | U | `theme::tests::tokens_match_tokens_css` (scrim alpha, both themes) | done |
| type-size tokens exist on :root | U | `theme::tests::tokens_match_tokens_css` (type sizes, spacing, radius, fonts) | done |
| every tarpack.css selector is scoped under .tarpack, and only the overlays have a shadow | - | CSS structure, no CSS in the GPUI app | dropped |

## `tools/pipeline/PipelineView.test.tsx` (19)

| Case | Owner | New test | Status |
|---|---|---|---|
| cleanPath trims and drops Explorer's quotes | M5 | `tools::pipeline::tests::clean_path_trims_and_unquotes` | done |
| lists the four steps in order and blocks running without a folder | M5 | `tools::pipeline::tests::run_reason_and_can_run`, `text_covers_every_error_kind_and_step` | done |
| lists the four steps in order and blocks running without a folder (view: the list's rendering and its axe check) | U | `tools::pipeline::view::tests::renders_each_fixture_state`, `step_list_shows_lock_lines_and_note`, `run_buttons_disabled_with_reason` (the axe check: dropped, labels and tab stops are tested instead) | done |
| browsing sets the folder and shows its subfolder count | M5 | `tools::pipeline::tests::browse_uses_folder_spec_and_cancel_changes_nothing`, `typed_folder_sets_directory` | done |
| browsing shows the subfolder count (view) | U | `tools::pipeline::view::tests::step_list_shows_lock_lines_and_note` (the note), `renders_each_fixture_state` | done |
| after run next, focus returns to run next for the following step | M5 | `tools::pipeline::tests::focus_hint_after_run_next` (moving focus is U, from `take_focus_hint`) | done |
| a step started from its own button does not show run next as busy | M5 | `tools::pipeline::tests::run_next_busy_only_for_its_step` | done |
| a typed bad path is shown with its error | M5 | `tools::pipeline::tests::bad_folder_shows_slot_error_and_announces` | done |
| a typed bad path is marked aria-invalid and described by its error (view) | U | `tools::pipeline::view::tests::renders_each_fixture_state` (`Bad`: error line and the kept path; aria-describedby: dropped) | done |
| the check step runs without confirming and shows its result | M5 | `tools::pipeline::tests::step_without_confirm_runs_immediately` | done |
| the check step's result is rendered (view) | U | `tools::pipeline::view::tests::renders_each_fixture_state` (`Ran`, `Demo`), `failed_step_shows_details` | done |
| issues are listed with their subfolder | U | `tools::pipeline::view::tests::renders_each_fixture_state` (`Demo`: the problems list) | done |
| the batch step asks first, and cancelling runs nothing | M5 | `tools::pipeline::tests::step_with_confirm_waits_and_uses_the_confirmed_folder`, `cancel_runs_nothing` | done |
| run all goes in order and stops at the first step that does not pass | M5 | `tools::pipeline::tests::run_all_confirms_once_and_stops_at_first_failure`, `run_all_all_passed_announces` | done |
| a step that cannot start shows an error banner | M5 | `tools::pipeline::tests::begin_error_is_a_banner_and_step_failure_is_not` (the banner's rendering is U) | done |
| focus moves to the step while it runs, not to the page | U | `tools::pipeline::view::tests::focus_moves_to_step_when_it_starts_running`, `focus_follows_hint_after_run_next` | done |
| a step started before the view mounted is shown and followed until it ends | M5 | dropped: the orphan polling (a GPUI model outlives its view and lands the result itself); kept behaviour: `tools::pipeline::tests::activated_clears_transient_state_and_keeps_running_step` | dropped |
| results, problems, failures and the confirmation have no axe violations | U | dropped: no axe in GPUI; labels, roles and focus are covered by `tools::pipeline::view::tests::confirming_step_opens_dialog_and_cancel_runs_nothing`, `renders_each_fixture_state` | dropped |
| a failed poll is reported once and retried | M5 | dropped: the orphan polling; a failed refresh is the banner (`load_failure_shows_retry_and_retry_loads`) | dropped |
| later steps are locked until the one before passes | M5 | `tools::pipeline::tests::step_status_covers_every_case` (`Waiting`), `locked_step_is_refused_without_the_view` | done |
| later steps show "Runs after <title> passes." and a disabled Run button (view) | U | `tools::pipeline::view::tests::step_list_shows_lock_lines_and_note`, `run_buttons_disabled_with_reason` | done |
| run next step walks through the steps one at a time | M5 | `tools::pipeline::tests::focus_hint_after_run_next` (walks every step with `request_next`) | done |
| when every step has passed, run next is disabled and says why | M5 | `tools::pipeline::tests::run_all_all_passed_announces` (`all_passed`, no run reason; the description wiring is U) | done |
| a repeated step result is re-announced even when the frame fires before React renders | M5 | `tools::pipeline::tests::repeated_result_bumps_announcement_generation` (the view re-announces on `announcement_generation`; the announcer hook is U) | done |

## `tools/schedule/ScheduleView.test.tsx` (20)

| Case | Owner | New test | Status |
|---|---|---|---|
| cleanPath trims and drops Explorer's quotes | M6 | `tools::schedule::tests::typed_paths_set_slots` (quotes and blanks; `app::path::clean_path` is also covered by `tools::pipeline::tests::clean_path_trims_and_unquotes`) | done |
| empty state: two labelled fields, Add disabled with a reason | M6 | `tools::schedule::tests::apply_reason_and_can_apply` | done |
| empty state: two labelled fields, heading and axe check (view half) | U | `tools::schedule::view::tests::renders_each_fixture_state` (both fields labelled through `FmInput::label`; axe has no GPUI equivalent) | done |
| a typed path is set on Enter, cleaned of quotes | M6 | `tools::schedule::tests::typed_paths_set_slots` (cleaned of quotes) | done |
| a typed path is set on Enter: the Enter key (view half) | U | `tools::schedule::view::tests::enter_and_blur_commit_typed_paths` | done |
| a typed path is set when leaving the field, and not resent unchanged | M6 | `tools::schedule::tests::typed_paths_set_slots` (an unchanged path sends nothing) | done |
| a typed path is set when leaving the field: blur (view half) | U | `tools::schedule::view::tests::enter_and_blur_commit_typed_paths` | done |
| emptying a field clears the file | M6 | `tools::schedule::tests::typed_paths_set_slots` (empty clears) | done |
| Escape restores the field | U | `tools::schedule::view::tests::escape_reverts_typing` | done |
| a bad path stays in the field, marked invalid with its error | M6 | `tools::schedule::tests::fixtures_build_each_screenshot_state` (`bad`: the slot keeps the path and its error) | done |
| a bad path stays in the field: invalid marking and description (view half) | U | `tools::schedule::view::tests::renders_each_fixture_state` (`bad`: the error line, field marked `.invalid`) | done |
| Browse opens the dialog with the right filters | M6 | `tools::schedule::tests::browse_passes_todays_filters`, `browse_sets_slot_and_cancel_does_nothing` | done |
| dropping files sets them and announces what was set | M6 | `tools::schedule::tests::drop_sets_slots_and_announces` | done |
| an ambiguous drop is reported | M6 | `tools::schedule::tests::ambiguous_drop_is_error` | done |
| adding confirms first, then reports the backup and moves focus to it | M6 | `tools::schedule::tests::apply_confirms_with_shown_paths_and_applies_exactly_those`, `result_sentence_after_apply`, `same_pair_twice_warns_duplicate` (focus hint `Outcome`) | done |
| adding confirms first: focus moves to the outcome banner (view half) | U | `tools::schedule::view::tests::focus_follows_hint_after_apply`, `result_banner_with_backup_line_dismisses` | done |
| cancelling the confirmation writes nothing | M6 | `tools::schedule::tests::cancel_applies_nothing` | done |
| a parse error from Add names the line and takes focus | M6 | `tools::schedule::tests::text_covers_every_error_kind` (the line copy), `apply_error_is_banner_and_session_kept` (focus hint `Outcome`) | done |
| the stub hooks say they are not implemented | M6 | `tools::schedule::tests::real_stub_hooks_change_nothing`, `apply_error_is_banner_and_session_kept` | done |
| an unknown rejection claims nothing about the files | M6 | `tools::schedule::tests::unexpected_command_failure_refetches_the_session`, `unexpected_apply_failure_refetches_the_session` (`Unexpected` copy, nothing claimed about the files) | done |
| a failed first load can be retried | M6 | `tools::schedule::tests::loads_session_and_retries_after_failure` | done |
| drops are ignored while the confirmation is open | M6 | `tools::schedule::tests::drop_target_enabled_and_disabled_reasons` | done |
| a late response from an older command is ignored | M6 | `tools::schedule::tests::only_newest_command_applies` | done |
| a typed path's error is announced | M6 | `tools::schedule::tests::typed_path_error_is_announced` | done |
| the same error from a second typed path is re-announced even when the frame fires before React renders | M6 | `tools::schedule::tests::typed_path_error_is_announced` (`announcement_generation` bumps) | done |
| the same error from a second typed path: the announcer hook (view half) | U | announcements are not rendered in the GPUI app (no live region); model half is `typed_path_error_is_announced` | done |

## `tools/tarpack/BuildBar.test.tsx` (15)

| Case | Owner | New test | Status |
|---|---|---|---|
| disabled reason: %s | U11 | model half: `tools::tarpack::tests::reason_and_left_out_note_cases` (the `build_reason` text, with `can_build`); the disabled button, its description and axe are the view's | open |
| Show errors only with errors, and none when noEntries without errors | U11 | model half: `tools::tarpack::tests::shortcuts_gating_matches_today` (`Shortcut::ShowErrors` needs errors); whether the button is drawn is the view's | open |
| ready with failed entries: enabled, note, Show errors before Create | U11 | model half: `tools::tarpack::tests::reason_and_left_out_note_cases`, `tools::tarpack::tests::shortcuts_gating_matches_today` (`left_out_note`, `can_build`); the order of the buttons is the view's | open |
| plural left-out note | M7b | `tools::tarpack::tests::reason_and_left_out_note_cases` | done |
| manifest errors only | M7b | `tools::tarpack::tests::reason_and_left_out_note_cases` | done |
| one manifest error | M7b | `tools::tarpack::tests::reason_and_left_out_note_cases` | done |
| nothingLoaded with errors shows reason and note | M7b | `tools::tarpack::tests::reason_and_left_out_note_cases` | done |
| a partial build is enabled and says how many files are not included | M7b | `tools::tarpack::tests::reason_and_left_out_note_cases`, `tools::tarpack::tests::build_refused_unless_can_build` | done |
| Choose calls the save dialog with the format's filter, then onChooseOutput | M7b | `tools::tarpack::tests::choose_output_passes_format_filter`, `tools::tarpack::tests::choose_output_starts_in_output_folder_with_name`, `tools::tarpack::tests::output_name_changing_format_sets_notice` | done |
| Choose falls back to the suggested name; cancel does nothing | M7b | `tools::tarpack::tests::choose_output_starts_in_output_folder_with_name`, `tools::tarpack::tests::choose_output_needs_a_manifest_and_a_name`, `tools::tarpack::tests::output_name_changing_format_sets_notice` (the cancel at its end) | done |
| no manifest: picker shows tar (.tar) and everything is disabled | U11 | model half: `tools::tarpack::tests::choose_output_needs_a_manifest_and_a_name`, `tools::tarpack::tests::build_refused_unless_can_build`; the picker's display and the disabled controls are the view's | open |
| the output path is a MiddlePath with the full path as title | U11 | view: renders `session().output_path`, the full path | open |
| building (%s): controls disabled, axe clean | U11 | model half: `tools::tarpack::tests::build_holds_without_blocking_the_ui` (every control's method refuses while `building()`); disabled controls and axe are the view's | open |
| saveFileDialog, onChooseOutput, and onFormatChange rejections go to onError | M7b | `tools::tarpack::tests::output_dialog_failure_is_the_dialog_banner`, `tools::tarpack::tests::output_and_format_errors_show_the_banner` | done |
| while building, Create archive's aria-describedby is omitted | U11 | view: draws the reason and the note only while `!building()` | open |

## `tools/tarpack/BuildReport.test.tsx` (3)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders nothing when everything is empty | U11 | view (the headings are `text::report_*_heading`) | open |
| each section is present only when non-empty; entry without id; every error | U11 | view; the copy is `text::failure_name`, `failure_errors_label` and `report_text` (`tools::tarpack::tests::report_text_matches_today`) | open |
| warnings only | U11 | view; the copied form is `tools::tarpack::tests::report_text_matches_today` | open |

## `tools/tarpack/BuildResult.test.tsx` (8)

| Case | Owner | New test | Status |
|---|---|---|---|
| shows every field and copies exact text | U11 | model half: `tools::tarpack::tests::copy_report_copies_report_text`, `tools::tarpack::tests::extraction_command_only_in_dev_mode`, `tools::tarpack::tests::report_text_matches_today` (the facts, sizes and the copied text); drawing the fields is the view's | open |
| tar shows one size; no normalised list when empty | U11 | model half: `tools::tarpack::tests::report_text_matches_today` (`result_facts`, `file_size`); view draws it | open |
| success with warnings only shows a collapsed Warnings disclosure | U11 | view | open |
| files left out: heading, lede, report, Copy report | U11 | model half: `tools::tarpack::tests::report_text_matches_today`, `tools::tarpack::tests::copy_report_copies_report_text` (heading, ledes, copied report); view draws it | open |
| plural files and manifest-errors-only headings | M7b | `tools::tarpack::tests::report_text_matches_today` | done |
| Show in folder calls onReveal | M7b | `tools::tarpack::tests::reveal_uses_held_path` (the button calling `reveal_output` is the view's own test) | done |
| error result uses errorMessage and collapsed details | U11 | model half: `tools::tarpack::tests::build_error_is_result_with_message` (the sentence and the detail); the collapsed Details are the view's | open |
| a rejected clipboard write says Couldn't copy, never Copied | M7b | dropped: GPUI's clipboard write reports no failure (`Platform::copy_text` returns nothing); "Copied" is the view's transient note (`text::COPIED`) | dropped |

## `tools/tarpack/DiagnosticList.test.tsx` (2)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders nothing for an empty list | U | dropped: the errors and warnings regions render only when there is something to list (`tools::tarpack::view::tests::renders_loaded_manifest`) | dropped |
| items have accessible position text and verbatim message | U | dropped: the list is labelled and each row shows position and message as text; no hidden spoken text in GPUI | dropped |

## `tools/tarpack/DropResult.test.tsx` (18)

| Case | Owner | New test | Status |
|---|---|---|---|
| matched line | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| unmatched reason %s has its copy and icon | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| plural phrases | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| two items with one reason share one line | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| ambiguous: an entry with two candidates | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| ambiguous: two entries sharing one candidate | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| ambiguous: an id not in entries falls back to the file name, never the id | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| notUnicode plural copy | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| U+FFFD paths render | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| more than 8 names: 8 shown, full count, and the same in dropResultText | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| failed entries: noEntry reads not matched and the hint follows; also after folderNoMatch alone | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| combined outcome: order, and dropResultText matches the rendered lines | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| dismiss by keyboard; no live role; axe clean | U | `tools::tarpack::view::tests::drop_result_lines_and_dismiss` (no live role: the result is not announced by the view; axe dropped) | done |
| an outcome with nothing in it says so | M7a | `tools::tarpack::tests::drop_lines_match_today` | done |
| empty outcome: text and icon are muted, no disclosure | U9b |  | open |
| Full paths: collapsed, expands, lists every path past the cap | U9b |  | open |
| Full paths is absent for matched-only outcomes | U9b |  | open |
| DropPending: one muted line, no button, no live role | U | `tools::tarpack::view::tests::drop_pending_line_after_the_delay` | done |

## `tools/tarpack/EntryStatus.test.tsx` (1)

| Case | Owner | New test | Status |
|---|---|---|---|
| %s shows %s | U10 |  | open |

## `tools/tarpack/EntryTable.keyboard.test.tsx` (9)

| Case | Owner | New test | Status |
|---|---|---|---|
| exactly one row is in the tab order, and Up and Down move focus between rows | U10 |  | open |
| Enter browses the focused row; Delete clears it only when assigned | U10 |  | open |
| Enter on a row's own button is that button's click, not a second browse | U10 |  | open |
| row actions carry shortcut hints and keep their names | U10 |  | open |
| Escape with focus on the drop result's Dismiss moves focus to the tab-stop row | U10 |  | open |
| Escape with focus elsewhere dismisses the result and leaves focus where it was | U10 |  | open |
| only the tab-stop row's own buttons are tabbable | U10 |  | open |
| disabled: no row or row button is tabbable, row keys do nothing, and the tab stop comes back | U10 |  | open |
| Clear leaves focus on the row: from its button by Delete and by click, and from the row itself | U10 |  | open |

## `tools/tarpack/EntryTable.scale.test.tsx` (1)

| Case | Owner | New test | Status |
|---|---|---|---|
| 2,000 rows render; one status change re-renders one row | U10 |  | open |

## `tools/tarpack/EntryTable.test.tsx` (11)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders a captioned table with column headers and one row per entry | U10 |  | open |
| summary: mixed | U10 |  | open |
| summary: all ready, with and without failed entries | U10 |  | open |
| line-ending clause and marker only for normalizeEol rows | U10 |  | open |
| empty cases render no table | U10 |  | open |
| Clear is disabled when unassigned; buttons call props with the id | U10 |  | open |
| action names use the target, so repeated sources stay distinct; Clear has a title | U10 |  | open |
| seven classed columns; cell labels are aria-hidden | U10 |  | open |
| target path span is mono, exact, with a wbr after each slash; no td is mono | U10 |  | open |
| Windows location: full path is the accessible text once, including for a long path | U10 |  | open |
| has no axe violations | U | dropped: axe is a DOM check; the nav's labels are the buttons' accessibility labels | dropped |

## `tools/tarpack/FailureList.test.tsx` (3)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders nothing for an empty list | U11 | view | open |
| names by id, by index, and shows source and line | U11 | view; the copy is `text::failure_name` and the `FAILURE_*` constants (M7a) | open |
| lists every error of a multi-error entry | U11 | view; the label is `text::failure_errors_label` (M7a) | open |

## `tools/tarpack/FormatPicker.test.tsx` (3)

| Case | Owner | New test | Status |
|---|---|---|---|
| options in the given order with session extensions; selected value | U11 | model half: `tools::tarpack::tests::format_labels_match_today` (`format_option_label`); the picker is the view's | open |
| keyboard selection calls onChange with the format | U11 | view calls `set_format` (`tools::tarpack::tests::notice_cleared_by_later_change` drives it) | open |
| disabled | U11 | view: disabled while `building()` or with no manifest | open |

## `tools/tarpack/ManifestErrors.test.tsx` (9)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders nothing with no errors or warnings | U | `tools::tarpack::view::tests::renders_loaded_manifest` | done |
| count heading, singular and plural | M7a | `tools::tarpack::tests::manifest_copy_matches_today` | done |
| keyed on errorCount, not errors.length | U | `tools::tarpack::view::tests::renders_errors_withheld_and_changed` (`Withheld`) | done |
| consequence lines | M7a | `tools::tarpack::tests::manifest_copy_matches_today` | done |
| report groups: whole manifest, then files with errors | M7a | `tools::tarpack::tests::manifest_copy_matches_today` | done |
| toggle has aria-expanded/controls and works by keyboard | U | `tools::tarpack::view::tests::errors_toggle_by_keyboard` (aria-expanded and controls dropped: the label flips Show errors / Hide errors) | done |
| collapsed body stays mounted but hidden | U | dropped: a collapsed report is not drawn (`tools::tarpack::view::tests::errors_toggle_by_keyboard` asserts it) | dropped |
| warnings render collapsed alongside errors, verbatim | U9b |  | open |
| warnings only | U9b |  | open |

## `tools/tarpack/ManifestHeader.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| each action calls its handler | U | `tools::tarpack::view::tests::header_actions_call_model`, `tools::tarpack::view::tests::edit_button_follows_dev_mode` | done |
| Recent is hidden when empty | U | `tools::tarpack::view::tests::recent_menu_hidden_when_empty` | done |
| Recent menu opens, moves, and closes with Escape returning focus | U | `tools::tarpack::view::tests::header_actions_call_model` (opens and picks by keys); the menu's own keys: `widgets::tests::menu_opens_with_keys_wraps_and_picks`, `menu_escape_refocuses_trigger` | done |
| Tab closes the menu and focus is not lost to body | U | `widgets::tests::menu_escape_refocuses_trigger` (Tab and Escape return focus to the trigger) | done |

## `tools/tarpack/MiddlePath.test.tsx` (2)

| Case | Owner | New test | Status |
|---|---|---|---|
| line is aria-hidden; full path is the accessible text and the title | U | `widgets::tests::middle_path_tooltip_is_full_path` (the tooltip is the full path; aria-hidden dropped: screen reader) | done |
| head plus tail equal the path when the name is under the cap | U | `widgets::middle_path::tests::middle_truncate_cases` (text that fits comes back whole; the name is kept whole) | done |

## `tools/tarpack/ShortcutsHelp.test.tsx` (3)

| Case | Owner | New test | Status |
|---|---|---|---|
| opens from the button by keyboard and lists every shortcut, including F8 | U9b |  | open |
| Escape closes it and returns focus to the button, without reaching the view | U9b |  | open |
| reports its open state, including false on unmount | U9b |  | open |

## `tools/tarpack/TarpackView.a11y.test.tsx` (10)

| Case | Owner | New test | Status |
|---|---|---|---|
| no manifest | dropped or U |  | open |
| errors with entries shown: report expanded, then collapsed | dropped or U |  | open |
| errors with entries withheld | dropped or U |  | open |
| partial and ready | dropped or U |  | open |
| ready with files left out | dropped or U |  | open |
| building: writing, then verifying | dropped or U |  | open |
| success with the extraction command and normalised entries | dropped or U |  | open |
| success with files left out, with the report | dropped or U |  | open |
| build error | dropped or U |  | open |
| shortcuts popover open | dropped or U |  | open |

## `tools/tarpack/TarpackView.assign.test.tsx` (21)

| Case | Owner | New test | Status |
|---|---|---|---|
| drop calls assignDropped with the dropped paths, renders session and outcome, announces the result | M7a | `tools::tarpack::tests::drop_assigns_and_reports_lines` | done |
| overlay axe check while visible | U10 |  | open |
| Browse assigns the chosen file, starting in the current folder | M7a | `tools::tarpack::tests::browse_starts_in_assigned_folder` | done |
| Browse on an unassigned row has no default path; cancel makes no call | M7a | `tools::tarpack::tests::browse_starts_in_assigned_folder` | done |
| Clear calls clear with the id and keeps its title | M7a | `tools::tarpack::tests::assign_and_clear_update_entries` | done |
| a NotAFile rejection shows the copy and leaves the table unchanged | M7a | `tools::tarpack::tests::assign_and_clear_update_entries` | done |
| a non-TarpackError rejection from assign shows the Io copy | - | dropped: a typed `Result<_, TarpackError>` cannot carry a non-TarpackError; a panicked backend call is `Unexpected` | dropped |
| a rejected drop shows an error banner and no result | M7a | `tools::tarpack::tests::drop_error_clears_pending` | done |
| with errorCount > 0 and passed entries, drop and Browse work | M7a | `tools::tarpack::tests::drop_with_failed_entries_keeps_the_flag` | done |
| silence on assign: Browse and Clear leave the announcer alone; a drop announces only its result | M7a | `tools::tarpack::tests::assign_and_clear_update_entries`, `tools::tarpack::tests::drop_assigns_and_reports_lines` | done |
| disabled (%s): overlay shows the reason and a drop makes no call | M7a | `tools::tarpack::tests::drop_target_reasons` | done |
| building disables drops | M7a | `tools::tarpack::tests::drop_target_reasons` | done |
| focus after Dismiss goes to the first tabbable after the result (the build bar) | U10 |  | open |
| one drop at a time: pending state, disabled reason, 150 ms line, 1 s announcement, resolve | M7a | `tools::tarpack::tests::drop_ignored_while_pending`, `tools::tarpack::tests::pending_shown_after_150ms_and_announced_after_1s` | done |
| a rejected drop clears the previous result and shows the banner | M7a | `tools::tarpack::tests::drop_error_clears_pending` | done |
| a result keeps its hasFailedEntries when the session later changes | M7a | `tools::tarpack::tests::drop_with_failed_entries_keeps_the_flag` | done |
| view-level axe with the result visible and Full paths expanded | U10 |  | open |
| the file list stays outside the controls fieldset and survives a drop | U10 |  | open |
| during a build the row buttons are disabled | U10 |  | open |
| Reload clears the drop result | M7a | `tools::tarpack::tests::reload_clears_changed_and_results` | done |
| Reload with focus in the drop result moves focus to Reload | M7a | `tools::tarpack::tests::reload_clears_changed_and_results` | done |

## `tools/tarpack/TarpackView.build.test.tsx` (24)

| Case | Owner | New test | Status |
|---|---|---|---|
| happy path: build(false), result, announcement, Show in folder | M7b | `tools::tarpack::tests::build_success_sets_result_and_announces`, `tools::tarpack::tests::reveal_uses_held_path` | done |
| setFormat re-renders the output path from the returned session | M7b | `tools::tarpack::tests::choose_output_passes_format_filter` | done |
| setOutput returning a switched format announces it | M7b | `tools::tarpack::tests::output_name_changing_format_sets_notice` (`format_notice`, read out by the view's live region) | done |
| OutputExists then Cancel makes no further call | M7b | `tools::tarpack::tests::cancel_overwrite_writes_nothing` | done |
| OutputExists then Replace calls build(true) | M7b | `tools::tarpack::tests::existing_output_asks_and_confirm_overwrites`, `tools::tarpack::tests::replace_asks_once_with_the_fixture` | done |
| ManifestChangedOnDisk shows the banner with Reload, not a result | M7b | `tools::tarpack::tests::changed_on_disk_during_build_marks_changed` (the banner itself is `changed_on_disk`, M7a) | done |
| SourceMissing names the file; without entryId says A file | M7b | `tools::tarpack::tests::build_error_is_result_with_message`, `tools::tarpack::tests::text_covers_every_error_kind` (the named file and the "A file" form) | done |
| %s shows its message and details; axe clean | U11 | model half: `tools::tarpack::tests::build_error_is_result_with_message`, `tools::tarpack::tests::panicked_build_aborts_and_reports`; the rendering and axe are the view's | open |
| progress: two phases, reset, Finishing, then the result | M7b | `tools::tarpack::tests::build_runs_off_thread_with_ordered_progress`, `tools::tarpack::tests::progress_names_the_last_file_of_the_phase`, `tools::tarpack::tests::progress_text_matches_today` | done |
| a rejection mid-verify stops the progress and shows the error | M7b | `tools::tarpack::tests::build_error_is_result_with_message` | done |
| OpenerFailed from Show in folder is a banner | M7b | `tools::tarpack::tests::reveal_failure_shows_opener_failed` | done |
| Show errors in the bar focuses the error report | M7b | `tools::tarpack::tests::shortcuts_run_what_they_gate`, `tools::tarpack::tests::show_errors_expands_and_hints_focus` (`FocusHint::Report`) | done |
| Show errors in the bar focuses the error report (view half) | U | `tools::tarpack::view::tests::focus_follows_hint` (the report half; the bar's button is U11) | done |
| building with errors: no dialog, build(false) once, left-out result and announcement | M7b | `tools::tarpack::tests::build_with_errors_reports_left_out` | done |
| a failed %s build announces its errorMessage | M7b | `tools::tarpack::tests::build_error_is_result_with_message`, `tools::tarpack::tests::panicked_build_aborts_and_reports` | done |
| %s announces no error message | M7b | `tools::tarpack::tests::existing_output_asks_and_confirm_overwrites` (`OutputExists` says nothing), `tools::tarpack::tests::changed_on_disk_during_build_marks_changed` (only the changed-on-disk sentence) | done |
| onBuildProgress rejecting: the build still runs, no progressbar, unavailable text | M7b | dropped: GPUI's progress channel cannot fail before the job ends (the job owns the sender); the Tauri listen failure has no equivalent. `tools::tarpack::tests::progress_channel_closing_before_result_is_not_a_failure` shows a normal build whose channel closes first has no special state | dropped |
| during a build, header and table controls are disabled; describedby has no dangling ids | U11 | model half: `tools::tarpack::tests::build_holds_without_blocking_the_ui`; disabled controls and the ids are the view's | open |
| describedby ids all exist while building with errors | U11 | view | open |
| a saveFileDialog rejection from Choose shows the error banner | M7b | `tools::tarpack::tests::output_dialog_failure_is_the_dialog_banner` | done |
| setOutput and setFormat rejections show the error banner | M7b | `tools::tarpack::tests::output_and_format_errors_show_the_banner` | done |
| the replace dialog shows a lossy folder as is | M7b | `tools::tarpack::tests::display_paths_split_like_today` (`overwrite_body` over a path with U+FFFD) | done |
| after a build, focus moves to Create archive only if it was lost | M7b | `tools::tarpack::tests::build_success_sets_result_and_announces`, `tools::tarpack::tests::cancel_overwrite_writes_nothing` (`FocusHint::CreateArchive`, `CreateArchiveAfterDialog`) | done |
| after a build, focus moves to Create archive only if it was lost (view half) | U11 | view: moves focus on `FocusHint::CreateArchive` when focus was lost, and always on `CreateArchiveAfterDialog` | open |
| focus that is elsewhere when a build settles stays there | U11 | view: acts on the hint only when focus was lost | open |
| row keys do nothing during a build, and work again after it | U11 | model half: `tools::tarpack::tests::build_holds_without_blocking_the_ui` (`clear` and `browse` refuse while `building()`); the keys are the view's | open |

## `tools/tarpack/TarpackView.errors.test.tsx` (11)

| Case | Owner | New test | Status |
|---|---|---|---|
| restore announces %s | M7a | `tools::tarpack::tests::restores_last_manifest_on_start`, `tools::tarpack::tests::manifest_sentences_match_today` | done |
| open and reload announce, repeat on same count, and clear announces No errors | M7a | `tools::tarpack::tests::open_uses_platform_and_announces`, `tools::tarpack::tests::reload_clears_changed_and_results`; the repeat: `tools::tarpack::tests::a_reload_with_the_same_error_count_announces_again` (same sentence, `announcement_generation` +1 each time) | done |
| a repeated sentence is re-announced even when the frame fires before React renders | M7a | `tools::tarpack::tests::state_warning_announced_once_and_dismissal_sticks` | done |
| report re-expands after reload even if collapsed; no focus stolen | M7a | `tools::tarpack::tests::load_expands_errors_when_present` | done |
| showErrors expands and focuses the report body; no-op without errors | M7a | `tools::tarpack::tests::show_errors_expands_and_hints_focus` | done |
| showErrors does nothing with zero errors | M7a | `tools::tarpack::tests::show_errors_noop_without_errors` | done |
| announce() sets the announcer text | M7a | `tools::tarpack::tests::open_uses_platform_and_announces` | done |
| changed-on-disk is announced once, not again on a second event | M7a | `tools::tarpack::tests::watcher_marks_changed_on_disk_once`, `tools::tarpack::tests::inject_file_changed_marks_changed` | done |
| a restore with a state warning and errors makes one combined announcement | M7a | `tools::tarpack::tests::state_warning_announced_once_and_dismissal_sticks` | done |
| no focus change on open or reload with errors | M7a | `tools::tarpack::tests::load_expands_errors_when_present` | done |
| 200 failed entries: all render, the report is the only tab stop and scrolls | U9b |  | open |

## `tools/tarpack/TarpackView.states.test.tsx` (17)

| Case | Owner | New test | Status |
|---|---|---|---|
| loading: announcer mounted, hidden text, busy, skeleton delayed | M7a | `tools::tarpack::tests::skeleton_only_after_150ms` | done |
| no-manifest state opens via dialog with a toml filter | M7a | `tools::tarpack::tests::open_passes_manifest_filter` | done |
| loaded state has no error region and no axe violations | U | `tools::tarpack::view::tests::renders_loaded_manifest` (axe dropped) | done |
| errors with entries shown, and withheld | M7a | `tools::tarpack::tests::fixtures_build_each_manifest_state` | done |
| changed on disk banner appears, Reload clears it | M7a | `tools::tarpack::tests::reload_clears_changed_and_results` | done |
| ManifestUnreadable from Open keeps the session; Details is a disclosure | M7a | `tools::tarpack::tests::open_failure_keeps_the_session` | done |
| ManifestUnreadable from Reload | M7a | `tools::tarpack::tests::open_failure_keeps_the_session` | done |
| PathExists from Create from example leaves the no-manifest state | M7a | `tools::tarpack::tests::create_from_example_refuses_existing_file` | done |
| OpenerFailed from Edit in editor and state warning | M7a | `tools::tarpack::tests::edit_opens_manifest_path` | done |
| a rejected first session() shows the no-manifest state with the banner | M7a | `tools::tarpack::tests::restore_failure_sets_load_failed` | done |
| a non-TarpackError rejection shows the Io copy with the thrown text | - | dropped: a typed `Result` cannot carry a non-TarpackError; a panicked backend call is `Unexpected` (`restore_failure_sets_load_failed`) | dropped |
| a rejected dialog is reported, not left unhandled | M7a | `tools::tarpack::tests::open_error_shows_dialog_failure_copy` | done |
| banners render in fixed order and Details open passes axe | U9b |  | open |
| changed-on-disk banner has no axe violations | U | `tools::tarpack::view::tests::changed_banner_reload` (axe dropped; the banner and its Reload action are tested) | done |
| root section has the tarpack class in loading, no-manifest and loaded states | - | dropped: a CSS class of the web view | dropped |
| skeleton has no axe violations after the delay | U | `tools::tarpack::view::tests::skeleton_only_when_model_says` (axe dropped) | done |
| the same command error twice re-mounts the alert so it is announced again | M7a | `tools::tarpack::tests::open_failure_keeps_the_session` | done |

## `tools/tarpack/errorMessages.test.ts` (6)

| Case | Owner | New test | Status |
|---|---|---|---|
| %s has a message | M7a | `tools::tarpack::tests::text_covers_every_error_kind`, `tools::tarpack::tests::text_matches_ts_sources` | done |
| there are 17 kinds including NoEntries | M7a | `tools::tarpack::tests::text_covers_every_error_kind` | done |
| source is filled from the entry id | M7a | `tools::tarpack::tests::text_covers_every_error_kind` | done |
| reads 'A file' when the entry id is omitted or unknown | M7a | `tools::tarpack::tests::text_covers_every_error_kind` | done |
| toTarpackError passes a valid error through unchanged | - | dropped: no untyped JS rejection to normalise; the backend returns `Result<_, TarpackError>` | dropped |
| toTarpackError turns %s into Io | - | dropped: no untyped JS rejection to normalise; the backend returns `Result<_, TarpackError>` | dropped |

## `tools/tarpack/pathParts.test.ts` (14)

| Case | Owner | New test | Status |
|---|---|---|---|
| splits at the last separator | U | `widgets::middle_path::tests::middle_truncate_cases` (the separator and name are kept: `\gateway.exe`) | done |
| drive-root file | U | `widgets::middle_path::tests::middle_truncate_cases` (a short drive-root path is unchanged; the drive is kept when cut) | done |
| UNC prefix stays in head | U | `widgets::middle_path::tests::middle_truncate_cases` (UNC case) | done |
| UNC server and share names may hold astral or non-ASCII characters | U | dropped: there is no prefix parse any more (the start is plain text cut by `char`); the astral case of `middle_truncate_cases` covers the characters | dropped |
| no separator: head empty, tail whole | U | `widgets::middle_path::tests::middle_truncate_cases` (no separator: an even split) | done |
| 200-character path keeps drive in head and name in tail | U | `widgets::middle_path::tests::middle_truncate_cases` | done |
| name of exactly 32 code points is kept, 33 is capped | U | dropped: the fixed 32-code-point name cap is replaced by the room rule (the name is kept while the head and the ellipsis still fit, else an even split); `middle_truncate_cases` covers a too-long name | dropped |
| an astral character at the cap is not split | U | `widgets::middle_path::tests::middle_truncate_cases` (astral case: cuts fall between `char`s) | done |
| U+FFFD is preserved | U | `widgets::middle_path::tests::middle_truncate_cases` (U+FFFD case) | done |
| assignedFolder: drive root keeps its separator | M7a | `tools::tarpack::tests::browse_starts_in_the_parent_folder` (Windows paths run in the Windows job; the Unix job runs the `/` root) | done |
| assignedFolder: nested, forward-slash and UNC paths | M7a | `tools::tarpack::tests::browse_starts_in_the_parent_folder` (Windows job) | done |
| assignedFolder: undefined without a separator or with U+FFFD | M7a | `tools::tarpack::tests::browse_starts_in_the_parent_folder` (no parent: no start folder), `tools::tarpack::tests::browse_starts_in_a_folder_that_is_not_valid_unicode` (the stored path has nothing to refuse: a lossy display name still starts in its exact folder). `C:a.txt` is not ported: `Path::parent` gives `C:` and the dialog accepts it | done |
| fileName is what follows the last separator | U10 | (the entry table renders each row's file name; U10 ports this case as a view-helper test) | open |
| displayFolder shows a lossy folder as is; assignedFolder refuses it | U10 | `displayFolder` half: U10 renders the row's folder and ports this case. The `assignedFolder` half is `tools::tarpack::tests::browse_starts_in_a_folder_that_is_not_valid_unicode`: it no longer refuses, it uses the stored `PathBuf` | open |

## `tools/tarpack/reportText.test.ts` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| full report: heading, left out, manifest errors, warnings | M7b | `tools::tarpack::tests::report_text_matches_today` | done |
| empty sections are omitted | M7b | `tools::tarpack::tests::report_text_matches_today` | done |
| announcements | M7b | `tools::tarpack::tests::report_text_matches_today` | done |
| a partial build names the files with no source chosen | M7b | `tools::tarpack::tests::report_text_matches_today` | done |

## `tools/tarpack/useTarpackShortcuts.test.tsx` (19)

| Case | Owner | New test | Status |
|---|---|---|---|
| Ctrl+O opens the file dialog, with or without a manifest | M7b | `tools::tarpack::tests::shortcuts_gating_matches_today`, `tools::tarpack::tests::shortcuts_run_what_they_gate` | done |
| F5 and Ctrl+R reload and never reload the webview; inactive without a manifest they are still swallowed | M7b | `tools::tarpack::tests::shortcuts_gating_matches_today` (the gating); the webview-reload guard is dropped: there is no webview to reload | done |
| F5 with no manifest is prevented and does nothing | M7b | `tools::tarpack::tests::shortcuts_gating_matches_today` (does nothing); the prevent-default half is dropped: no webview | done |
| Ctrl+E edits in the editor only when a manifest is loaded | M7b | `tools::tarpack::tests::shortcuts_gating_matches_today` | done |
| Ctrl+E opens the editor with a manifest | M7b | `tools::tarpack::tests::shortcuts_run_what_they_gate` | done |
| Ctrl+Enter builds when canBuild, and does nothing when it is false | M7b | `tools::tarpack::tests::shortcuts_gating_matches_today`, `tools::tarpack::tests::build_refused_unless_can_build` | done |
| Ctrl+Enter builds a canBuild session that has errors, with no dialog | M7b | `tools::tarpack::tests::shortcuts_run_what_they_gate`, `tools::tarpack::tests::build_with_errors_reports_left_out` | done |
| F8 moves focus to the error report when there are errors, and does nothing otherwise | M7b | `tools::tarpack::tests::shortcuts_gating_matches_today`, `tools::tarpack::tests::shortcuts_run_what_they_gate` | done |
| Escape dismisses the build result, but never collapses the error report | M7b | `tools::tarpack::tests::escape_dismisses_drop_result_then_build_result` | done |
| shortcuts are ignored while a build is running | M7b | `tools::tarpack::tests::shortcuts_gating_matches_today` | done |
| shortcuts are ignored while the replace dialog is open | M7b | `tools::tarpack::tests::shortcuts_gating_matches_today` | done |
| buttons carry their shortcut hints | U11 | view: the hints are `text::SHOW_ERRORS_HINT`, `CREATE_ARCHIVE_HINT`, `OPEN_HINT` | open |
| tab order through the build bar: Choose…, Format, Show errors, Create archive | U11 | view | open |
| %s never reloads the webview and never runs Reload manifest | M7b | dropped: there is no webview to reload; the model's gating is `tools::tarpack::tests::shortcuts_gating_matches_today` | dropped |
| reload chords are prevented with no manifest too | M7b | dropped: there is no webview to reload; no reload without a manifest is `tools::tarpack::tests::shortcuts_gating_matches_today` | dropped |
| reload chords are prevented during a build | M7b | dropped: there is no webview to reload; no reload during a build is `tools::tarpack::tests::shortcuts_gating_matches_today` | dropped |
| Escape while the shortcuts popover is open neither dismisses a result nor is prevented | M7b | `tools::tarpack::tests::escape_ignored_while_shortcuts_open` (not dismissing; letting the key through is the view's binding) | done |
| without developer mode, Ctrl+E is claimed but opens nothing | M7b | `tools::tarpack::tests::shortcuts_gating_matches_today` (opens nothing; the claim is the view's binding) | done |
| Reload clears the build result | M7b | `tools::tarpack::tests::reload_clears_the_build_result` | done |
| Open, open recent and create from the example clear the build result (and its Show in folder) | M7b | changed: Load clears the result too (2026-11-22); the Tauri app kept it, so Show in folder could point at the previous manifest's archive. `tools::tarpack::tests::open_clears_build_result`, `open_recent_clears_build_result`, `create_from_example_clears_build_result` | done |
