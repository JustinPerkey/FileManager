# Schedule Creator

Reads a schedule from a text file and adds it to an existing XML file. The
user specifies both files.

The UI, the commands, the backup, and the write are done. The two domain
steps are stubs in `crates/fm-schedule` that return `NotImplemented`; the UI
reports "not implemented yet" when the user clicks Add.

## Flow

1. **Specify the schedule file and the XML file.** For each one the user can:
   - type or paste a path in its field (set on Enter or when leaving the
     field; surrounding quotes from Explorer's "Copy as path" are removed;
     an empty field clears the file; Escape restores the field);
   - click **Browse…** for the Open dialog;
   - drop it on the window (`schedule_set_dropped`): a `.xml` file becomes
     the XML file and any other file the schedule file. A drop can carry one
     of each; two of the same kind changes nothing and is reported.

   `schedule_set_text` / `schedule_set_xml` only check that the path is a
   file of UTF-8 text. Nothing is parsed yet. A bad path stays in its field,
   marked invalid with its error, and blocks Add.
2. **Add to XML…** is enabled once both files are set without errors. The
   user confirms (the dialog names both files, and warns if this schedule was
   already added to this file in this session), then `schedule_apply`:
   - reads the schedule file as it is now (UTF-8, BOM dropped) and calls
     `fm_schedule::parse`;
   - reads the XML as it is now (UTF-8, BOM dropped) and calls
     `fm_schedule::merge`;
   - copies the original XML bytes to the first free name of `<file>.bak`,
     `<file>.bak.1`, … (never replacing an existing file);
   - replaces the XML atomically (`fm_core::atomic_write`), writing back the
     byte-order mark if the original had one.

   Nothing is written when parsing or merging fails; the error is shown
   (with the line number for a parse error) and takes focus.

   The atomic replace creates a new file: the XML's ACLs and attributes are
   not carried over, and a symlink is replaced by a plain file.

## Where the work goes

| What | Where |
| --- | --- |
| The parsed data model | `Schedule` in `crates/fm-schedule/src/model.rs` (placeholder fields) |
| Parsing | `parse(text) -> Result<Schedule, ParseError>` in `src/parse.rs` |
| Updating the XML | `merge(schedule, xml) -> Result<MergeOutcome, MergeError>` in `src/merge.rs` |

- `ParseError::Invalid { line, message }` is shown as
  "Line N of the schedule file: message. Nothing was changed."
- `MergeOutcome { xml, added }`: `xml` is the whole new document; `added` is
  the count in the success message.
- `MergeError::InvalidXml` / `Conflict`: shown as "could not be added",
  with the message under Details.
- `merge` must not touch the filesystem.

`Schedule` never crosses to the UI, so its fields can change freely. Changing
a type in `src-tauri/src/tools/schedule/types.rs` is a boundary change:
regenerate `lib/generated/` and update the view.

The shell tests (`src-tauri/src/tools/schedule/tests.rs`) drive the whole flow
with fake hooks, so they keep passing whatever the real hooks do. Add the
parser's and merger's own tests in `fm-schedule`, and replace its
`stub_reports_not_implemented` tests when the stubs go.
