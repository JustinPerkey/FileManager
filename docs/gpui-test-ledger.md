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
file's table equals that count. The file counts sum to 301.


## `App.test.tsx` (1)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders the shell with the Pipeline view first | U |  | open |

## `app/AppShell.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders registry entries | M3a | `tools::tests::registry_order_and_labels` | done |
| selecting a tool renders its view | U |  | open |
| shows an empty state with no tools | U |  | open |
| has no axe violations | U |  | open |

## `app/Banner.test.tsx` (3)

| Case | Owner | New test | Status |
|---|---|---|---|
| %s tone has the right live role and hidden word | U |  | open |
| action and dismiss | U |  | open |
| renders children | U |  | open |

## `app/Button.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| variants | U |  | open |
| forwards ref and disabled | U |  | open |
| busy sets aria-busy and disables | U |  | open |
| icon renders aria-hidden and no axe violations | U |  | open |

## `app/ConfirmDialog.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| focus starts on Cancel, Escape cancels, focus returns | U |  | open |
| Replace confirms | U |  | open |
| no axe violations when open | U |  | open |
| aria-labelledby and aria-describedby resolve to the title and body, with unique ids | U |  | open |

## `app/DropZone.test.tsx` (6)

| Case | Owner | New test | Status |
|---|---|---|---|
| idle: renders nothing | M3b | `app::shell::tests::drag_over_tracks_enter_and_exit` (`drag_over()` is `None`; the chrome renders nothing from it) | done |
| hover shows the label on a plate; leave hides it | M3b, U | `app::shell::tests::drag_over_tracks_enter_and_exit` (state); the plate's look is U | done (state), open (look: U) |
| drop hides the overlay and passes exactly the dropped paths | M3b | `app::shell::tests::drop_reaches_current_tool_when_enabled`, `dropped_non_unicode_path_survives_routing` (Windows) | done |
| disabled: overlay shows the reason and a drop is ignored | M3b, U | `app::shell::tests::drop_ignored_when_disabled`, `empty_drop_ignored`; the reason's look is U | done (state), open (look: U) |
| does not steal focus, and unsubscribes on unmount | U |  | open (the shell's drop handlers are element listeners on the root div: no focus handling, nothing to unsubscribe) |
| after a rerender, drops follow the latest enabled and onDrop | M3b | `app::shell::tests::drop_goes_to_the_current_tool_only` (the target is read from the tool at drop time) | done |

## `app/ToolNav.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| marks the active item with aria-current | U |  | open |
| arrow keys move focus and wrap | U |  | open |
| Enter and Space select the focused item | U |  | open |
| the current item keeps aria-current and has no thick side border | U |  | open |

## `app/devMode.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| Ctrl+Alt+Shift+D toggles developer mode and announces it | M3a | `app::dev_mode::tests::chord_toggles` (announcement strings are constants in `app/text.rs`; the aria-live announcer itself is the UI's decision); focus independence: `chord_works_without_focus`, `chord_works_after_focus_is_lost`, `chord_works_with_focus_in_a_dialog`; held chord: `held_chord_toggles_once` | done |
| other chords leave developer mode off | M3a | `app::dev_mode::tests::chord_toggles` (the other-chord assertions) | done |
| without developer mode, Edit in editor and the extraction command are hidden | M7a or U |  | open |
| the shortcuts list names Ctrl+E only in developer mode | M7a or U |  | open |

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
| dark hooks define identical values | U |  | open |
| ${name}: %s on %s meets 4.5:1 | U |  | open |
| %s defines the overlay shadow | U |  | open |
| %s defines --scrim | U |  | open |
| type-size tokens exist on :root | U |  | open |
| every tarpack.css selector is scoped under .tarpack, and only the overlays have a shadow | U |  | open |

## `tools/pipeline/PipelineView.test.tsx` (18)

| Case | Owner | New test | Status |
|---|---|---|---|
| cleanPath trims and drops Explorer's quotes | M5 or U |  | open |
| lists the four steps in order and blocks running without a folder | M5 or U |  | open |
| browsing sets the folder and shows its subfolder count | M5 or U |  | open |
| after run next, focus returns to run next for the following step | M5 or U |  | open |
| a step started from its own button does not show run next as busy | M5 or U |  | open |
| a typed bad path is shown with its error | M5 or U |  | open |
| the check step runs without confirming and shows its result | M5 or U |  | open |
| issues are listed with their subfolder | M5 or U |  | open |
| the batch step asks first, and cancelling runs nothing | M5 or U |  | open |
| run all goes in order and stops at the first step that does not pass | M5 or U |  | open |
| a step that cannot start shows an error banner | M5 or U |  | open |
| focus moves to the step while it runs, not to the page | M5 or U |  | open |
| a step started before the view mounted is shown and followed until it ends | M5 or U |  | open |
| results, problems, failures and the confirmation have no axe violations | M5 or U |  | open |
| a failed poll is reported once and retried | M5 or U |  | open |
| later steps are locked until the one before passes | M5 or U |  | open |
| run next step walks through the steps one at a time | M5 or U |  | open |
| when every step has passed, run next is disabled and says why | M5 or U |  | open |

## `tools/schedule/ScheduleView.test.tsx` (19)

| Case | Owner | New test | Status |
|---|---|---|---|
| cleanPath trims and drops Explorer's quotes | M6 or U |  | open |
| empty state: two labelled fields, Add disabled with a reason | M6 or U |  | open |
| a typed path is set on Enter, cleaned of quotes | M6 or U |  | open |
| a typed path is set when leaving the field, and not resent unchanged | M6 or U |  | open |
| emptying a field clears the file | M6 or U |  | open |
| Escape restores the field | M6 or U |  | open |
| a bad path stays in the field, marked invalid with its error | M6 or U |  | open |
| Browse opens the dialog with the right filters | M6 or U |  | open |
| dropping files sets them and announces what was set | M6 or U |  | open |
| an ambiguous drop is reported | M6 or U |  | open |
| adding confirms first, then reports the backup and moves focus to it | M6 or U |  | open |
| cancelling the confirmation writes nothing | M6 or U |  | open |
| a parse error from Add names the line and takes focus | M6 or U |  | open |
| the stub hooks say they are not implemented | M6 or U |  | open |
| an unknown rejection claims nothing about the files | M6 or U |  | open |
| a failed first load can be retried | M6 or U |  | open |
| drops are ignored while the confirmation is open | M6 or U |  | open |
| a late response from an older command is ignored | M6 or U |  | open |
| a typed path's error is announced | M6 or U |  | open |

## `tools/tarpack/BuildBar.test.tsx` (15)

| Case | Owner | New test | Status |
|---|---|---|---|
| disabled reason: %s | M7b or U |  | open |
| Show errors only with errors, and none when noEntries without errors | M7b or U |  | open |
| ready with failed entries: enabled, note, Show errors before Create | M7b or U |  | open |
| plural left-out note | M7b or U |  | open |
| manifest errors only | M7b or U |  | open |
| one manifest error | M7b or U |  | open |
| nothingLoaded with errors shows reason and note | M7b or U |  | open |
| a partial build is enabled and says how many files are not included | M7b or U |  | open |
| Choose calls the save dialog with the format's filter, then onChooseOutput | M7b or U |  | open |
| Choose falls back to the suggested name; cancel does nothing | M7b or U |  | open |
| no manifest: picker shows tar (.tar) and everything is disabled | M7b or U |  | open |
| the output path is a MiddlePath with the full path as title | M7b or U |  | open |
| building (%s): controls disabled, axe clean | M7b or U |  | open |
| saveFileDialog, onChooseOutput, and onFormatChange rejections go to onError | M7b or U |  | open |
| while building, Create archive's aria-describedby is omitted | M7b or U |  | open |

## `tools/tarpack/BuildReport.test.tsx` (3)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders nothing when everything is empty | M7b or U |  | open |
| each section is present only when non-empty; entry without id; every error | M7b or U |  | open |
| warnings only | M7b or U |  | open |

## `tools/tarpack/BuildResult.test.tsx` (8)

| Case | Owner | New test | Status |
|---|---|---|---|
| shows every field and copies exact text | M7b or U |  | open |
| tar shows one size; no normalised list when empty | M7b or U |  | open |
| success with warnings only shows a collapsed Warnings disclosure | M7b or U |  | open |
| files left out: heading, lede, report, Copy report | M7b or U |  | open |
| plural files and manifest-errors-only headings | M7b or U |  | open |
| Show in folder calls onReveal | M7b or U |  | open |
| error result uses errorMessage and collapsed details | M7b or U |  | open |
| a rejected clipboard write says Couldn't copy, never Copied | M7b or U |  | open |

## `tools/tarpack/DiagnosticList.test.tsx` (2)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders nothing for an empty list | M7a or U |  | open |
| items have accessible position text and verbatim message | M7a or U |  | open |

## `tools/tarpack/DropResult.test.tsx` (18)

| Case | Owner | New test | Status |
|---|---|---|---|
| matched line | M7a or U |  | open |
| unmatched reason %s has its copy and icon | M7a or U |  | open |
| plural phrases | M7a or U |  | open |
| two items with one reason share one line | M7a or U |  | open |
| ambiguous: an entry with two candidates | M7a or U |  | open |
| ambiguous: two entries sharing one candidate | M7a or U |  | open |
| ambiguous: an id not in entries falls back to the file name, never the id | M7a or U |  | open |
| notUnicode plural copy | M7a or U |  | open |
| U+FFFD paths render | M7a or U |  | open |
| more than 8 names: 8 shown, full count, and the same in dropResultText | M7a or U |  | open |
| failed entries: noEntry reads not matched and the hint follows; also after folderNoMatch alone | M7a or U |  | open |
| combined outcome: order, and dropResultText matches the rendered lines | M7a or U |  | open |
| dismiss by keyboard; no live role; axe clean | M7a or U |  | open |
| an outcome with nothing in it says so | M7a or U |  | open |
| empty outcome: text and icon are muted, no disclosure | M7a or U |  | open |
| Full paths: collapsed, expands, lists every path past the cap | M7a or U |  | open |
| Full paths is absent for matched-only outcomes | M7a or U |  | open |
| DropPending: one muted line, no button, no live role | M7a or U |  | open |

## `tools/tarpack/EntryStatus.test.tsx` (1)

| Case | Owner | New test | Status |
|---|---|---|---|
| %s shows %s | U |  | open |

## `tools/tarpack/EntryTable.keyboard.test.tsx` (9)

| Case | Owner | New test | Status |
|---|---|---|---|
| exactly one row is in the tab order, and Up and Down move focus between rows | U |  | open |
| Enter browses the focused row; Delete clears it only when assigned | U |  | open |
| Enter on a row's own button is that button's click, not a second browse | U |  | open |
| row actions carry shortcut hints and keep their names | U |  | open |
| Escape with focus on the drop result's Dismiss moves focus to the tab-stop row | U |  | open |
| Escape with focus elsewhere dismisses the result and leaves focus where it was | U |  | open |
| only the tab-stop row's own buttons are tabbable | U |  | open |
| disabled: no row or row button is tabbable, row keys do nothing, and the tab stop comes back | U |  | open |
| Clear leaves focus on the row: from its button by Delete and by click, and from the row itself | U |  | open |

## `tools/tarpack/EntryTable.scale.test.tsx` (1)

| Case | Owner | New test | Status |
|---|---|---|---|
| 2,000 rows render; one status change re-renders one row | U |  | open |

## `tools/tarpack/EntryTable.test.tsx` (11)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders a captioned table with column headers and one row per entry | U |  | open |
| summary: mixed | U |  | open |
| summary: all ready, with and without failed entries | U |  | open |
| line-ending clause and marker only for normalizeEol rows | U |  | open |
| empty cases render no table | U |  | open |
| Clear is disabled when unassigned; buttons call props with the id | U |  | open |
| action names use the target, so repeated sources stay distinct; Clear has a title | U |  | open |
| seven classed columns; cell labels are aria-hidden | U |  | open |
| target path span is mono, exact, with a wbr after each slash; no td is mono | U |  | open |
| Windows location: full path is the accessible text once, including for a long path | U |  | open |
| has no axe violations | U |  | open |

## `tools/tarpack/FailureList.test.tsx` (3)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders nothing for an empty list | M7b or U |  | open |
| names by id, by index, and shows source and line | M7b or U |  | open |
| lists every error of a multi-error entry | M7b or U |  | open |

## `tools/tarpack/FormatPicker.test.tsx` (3)

| Case | Owner | New test | Status |
|---|---|---|---|
| options in the given order with session extensions; selected value | M7b or U |  | open |
| keyboard selection calls onChange with the format | M7b or U |  | open |
| disabled | M7b or U |  | open |

## `tools/tarpack/ManifestErrors.test.tsx` (9)

| Case | Owner | New test | Status |
|---|---|---|---|
| renders nothing with no errors or warnings | M7a or U |  | open |
| count heading, singular and plural | M7a or U |  | open |
| keyed on errorCount, not errors.length | M7a or U |  | open |
| consequence lines | M7a or U |  | open |
| report groups: whole manifest, then files with errors | M7a or U |  | open |
| toggle has aria-expanded/controls and works by keyboard | M7a or U |  | open |
| collapsed body stays mounted but hidden | M7a or U |  | open |
| warnings render collapsed alongside errors, verbatim | M7a or U |  | open |
| warnings only | M7a or U |  | open |

## `tools/tarpack/ManifestHeader.test.tsx` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| each action calls its handler | M7a or U |  | open |
| Recent is hidden when empty | M7a or U |  | open |
| Recent menu opens, moves, and closes with Escape returning focus | M7a or U |  | open |
| Tab closes the menu and focus is not lost to body | M7a or U |  | open |

## `tools/tarpack/MiddlePath.test.tsx` (2)

| Case | Owner | New test | Status |
|---|---|---|---|
| line is aria-hidden; full path is the accessible text and the title | U |  | open |
| head plus tail equal the path when the name is under the cap | U |  | open |

## `tools/tarpack/ShortcutsHelp.test.tsx` (3)

| Case | Owner | New test | Status |
|---|---|---|---|
| opens from the button by keyboard and lists every shortcut, including F8 | U |  | open |
| Escape closes it and returns focus to the button, without reaching the view | U |  | open |
| reports its open state, including false on unmount | U |  | open |

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
| drop calls assignDropped with the dropped paths, renders session and outcome, announces the result | M7a or U |  | open |
| overlay axe check while visible | M7a or U |  | open |
| Browse assigns the chosen file, starting in the current folder | M7a or U |  | open |
| Browse on an unassigned row has no default path; cancel makes no call | M7a or U |  | open |
| Clear calls clear with the id and keeps its title | M7a or U |  | open |
| a NotAFile rejection shows the copy and leaves the table unchanged | M7a or U |  | open |
| a non-TarpackError rejection from assign shows the Io copy | M7a or U |  | open |
| a rejected drop shows an error banner and no result | M7a or U |  | open |
| with errorCount > 0 and passed entries, drop and Browse work | M7a or U |  | open |
| silence on assign: Browse and Clear leave the announcer alone; a drop announces only its result | M7a or U |  | open |
| disabled (%s): overlay shows the reason and a drop makes no call | M7a or U |  | open |
| building disables drops | M7a or U |  | open |
| focus after Dismiss goes to the first tabbable after the result (the build bar) | M7a or U |  | open |
| one drop at a time: pending state, disabled reason, 150 ms line, 1 s announcement, resolve | M7a or U |  | open |
| a rejected drop clears the previous result and shows the banner | M7a or U |  | open |
| a result keeps its hasFailedEntries when the session later changes | M7a or U |  | open |
| view-level axe with the result visible and Full paths expanded | M7a or U |  | open |
| the file list stays outside the controls fieldset and survives a drop | M7a or U |  | open |
| during a build the row buttons are disabled | M7a or U |  | open |
| Reload clears the drop result | M7a or U |  | open |
| Reload with focus in the drop result moves focus to Reload | M7a or U |  | open |

## `tools/tarpack/TarpackView.build.test.tsx` (24)

| Case | Owner | New test | Status |
|---|---|---|---|
| happy path: build(false), result, announcement, Show in folder | M7b or U |  | open |
| setFormat re-renders the output path from the returned session | M7b or U |  | open |
| setOutput returning a switched format announces it | M7b or U |  | open |
| OutputExists then Cancel makes no further call | M7b or U |  | open |
| OutputExists then Replace calls build(true) | M7b or U |  | open |
| ManifestChangedOnDisk shows the banner with Reload, not a result | M7b or U |  | open |
| SourceMissing names the file; without entryId says A file | M7b or U |  | open |
| %s shows its message and details; axe clean | M7b or U |  | open |
| progress: two phases, reset, Finishing, then the result | M7b or U |  | open |
| a rejection mid-verify stops the progress and shows the error | M7b or U |  | open |
| OpenerFailed from Show in folder is a banner | M7b or U |  | open |
| Show errors in the bar focuses the error report | M7b or U |  | open |
| building with errors: no dialog, build(false) once, left-out result and announcement | M7b or U |  | open |
| a failed %s build announces its errorMessage | M7b or U |  | open |
| %s announces no error message | M7b or U |  | open |
| onBuildProgress rejecting: the build still runs, no progressbar, unavailable text | M7b or U |  | open |
| during a build, header and table controls are disabled; describedby has no dangling ids | M7b or U |  | open |
| describedby ids all exist while building with errors | M7b or U |  | open |
| a saveFileDialog rejection from Choose shows the error banner | M7b or U |  | open |
| setOutput and setFormat rejections show the error banner | M7b or U |  | open |
| the replace dialog shows a lossy folder as is | M7b or U |  | open |
| after a build, focus moves to Create archive only if it was lost | M7b or U |  | open |
| focus that is elsewhere when a build settles stays there | M7b or U |  | open |
| row keys do nothing during a build, and work again after it | M7b or U |  | open |

## `tools/tarpack/TarpackView.errors.test.tsx` (10)

| Case | Owner | New test | Status |
|---|---|---|---|
| restore announces %s | M7a or U |  | open |
| open and reload announce, repeat on same count, and clear announces No errors | M7a or U |  | open |
| report re-expands after reload even if collapsed; no focus stolen | M7a or U |  | open |
| showErrors expands and focuses the report body; no-op without errors | M7a or U |  | open |
| showErrors does nothing with zero errors | M7a or U |  | open |
| announce() sets the announcer text | M7a or U |  | open |
| changed-on-disk is announced once, not again on a second event | M7a or U |  | open |
| a restore with a state warning and errors makes one combined announcement | M7a or U |  | open |
| no focus change on open or reload with errors | M7a or U |  | open |
| 200 failed entries: all render, the report is the only tab stop and scrolls | M7a or U |  | open |

## `tools/tarpack/TarpackView.states.test.tsx` (17)

| Case | Owner | New test | Status |
|---|---|---|---|
| loading: announcer mounted, hidden text, busy, skeleton delayed | M7a or U |  | open |
| no-manifest state opens via dialog with a toml filter | M7a or U |  | open |
| loaded state has no error region and no axe violations | M7a or U |  | open |
| errors with entries shown, and withheld | M7a or U |  | open |
| changed on disk banner appears, Reload clears it | M7a or U |  | open |
| ManifestUnreadable from Open keeps the session; Details is a disclosure | M7a or U |  | open |
| ManifestUnreadable from Reload | M7a or U |  | open |
| PathExists from Create from example leaves the no-manifest state | M7a or U |  | open |
| OpenerFailed from Edit in editor and state warning | M7a or U |  | open |
| a rejected first session() shows the no-manifest state with the banner | M7a or U |  | open |
| a non-TarpackError rejection shows the Io copy with the thrown text | M7a or U |  | open |
| a rejected dialog is reported, not left unhandled | M7a or U |  | open |
| banners render in fixed order and Details open passes axe | M7a or U |  | open |
| changed-on-disk banner has no axe violations | M7a or U |  | open |
| root section has the tarpack class in loading, no-manifest and loaded states | M7a or U |  | open |
| skeleton has no axe violations after the delay | M7a or U |  | open |
| the same command error twice re-mounts the alert so it is announced again | M7a or U |  | open |

## `tools/tarpack/errorMessages.test.ts` (6)

| Case | Owner | New test | Status |
|---|---|---|---|
| %s has a message | M7a or U |  | open |
| there are 17 kinds including NoEntries | M7a or U |  | open |
| source is filled from the entry id | M7a or U |  | open |
| reads 'A file' when the entry id is omitted or unknown | M7a or U |  | open |
| toTarpackError passes a valid error through unchanged | M7a or U |  | open |
| toTarpackError turns %s into Io | M7a or U |  | open |

## `tools/tarpack/pathParts.test.ts` (14)

| Case | Owner | New test | Status |
|---|---|---|---|
| splits at the last separator | U |  | open |
| drive-root file | U |  | open |
| UNC prefix stays in head | U |  | open |
| UNC server and share names may hold astral or non-ASCII characters | U |  | open |
| no separator: head empty, tail whole | U |  | open |
| 200-character path keeps drive in head and name in tail | U |  | open |
| name of exactly 32 code points is kept, 33 is capped | U |  | open |
| an astral character at the cap is not split | U |  | open |
| U+FFFD is preserved | U |  | open |
| assignedFolder: drive root keeps its separator | U |  | open |
| assignedFolder: nested, forward-slash and UNC paths | U |  | open |
| assignedFolder: undefined without a separator or with U+FFFD | U |  | open |
| fileName is what follows the last separator | U |  | open |
| displayFolder shows a lossy folder as is; assignedFolder refuses it | U |  | open |

## `tools/tarpack/reportText.test.ts` (4)

| Case | Owner | New test | Status |
|---|---|---|---|
| full report: heading, left out, manifest errors, warnings | M7b or U |  | open |
| empty sections are omitted | M7b or U |  | open |
| announcements | M7b or U |  | open |
| a partial build names the files with no source chosen | M7b or U |  | open |

## `tools/tarpack/useTarpackShortcuts.test.tsx` (19)

| Case | Owner | New test | Status |
|---|---|---|---|
| Ctrl+O opens the file dialog, with or without a manifest | M7b or U |  | open |
| F5 and Ctrl+R reload and never reload the webview; inactive without a manifest they are still swallowed | M7b or U |  | open |
| F5 with no manifest is prevented and does nothing | M7b or U |  | open |
| Ctrl+E edits in the editor only when a manifest is loaded | M7b or U |  | open |
| Ctrl+E opens the editor with a manifest | M7b or U |  | open |
| Ctrl+Enter builds when canBuild, and does nothing when it is false | M7b or U |  | open |
| Ctrl+Enter builds a canBuild session that has errors, with no dialog | M7b or U |  | open |
| F8 moves focus to the error report when there are errors, and does nothing otherwise | M7b or U |  | open |
| Escape dismisses the build result, but never collapses the error report | M7b or U |  | open |
| shortcuts are ignored while a build is running | M7b or U |  | open |
| shortcuts are ignored while the replace dialog is open | M7b or U |  | open |
| buttons carry their shortcut hints | M7b or U |  | open |
| tab order through the build bar: Choose…, Format, Show errors, Create archive | M7b or U |  | open |
| %s never reloads the webview and never runs Reload manifest | M7b or U |  | open |
| reload chords are prevented with no manifest too | M7b or U |  | open |
| reload chords are prevented during a build | M7b or U |  | open |
| Escape while the shortcuts popover is open neither dismisses a result nor is prevented | M7b or U |  | open |
| without developer mode, Ctrl+E is claimed but opens nothing | M7b or U |  | open |
| Reload clears the build result | M7b or U |  | open |
