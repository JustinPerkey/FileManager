# Schedule Creator

Reads a schedule from a text file and adds it to an existing XML file.

The UI, the commands, the backup, and the write are done. The two domain
steps are stubs in `crates/fm-schedule` that return `NotImplemented`; the UI
shows "not implemented yet" until they are written.

## Flow

1. **Choose schedule file** (`schedule_open_text`). The shell reads the file as
   UTF-8 (a BOM is dropped) and calls `fm_schedule::parse`. A parse error is
   kept in the session and shown under the file, with its line number;
   **Reload** (`schedule_reload_text`) reads the file again.
2. **Choose XML file** (`schedule_open_xml`). Only checked to be a file here.
3. **Add to XML…** is enabled once the text parsed and an XML file is chosen.
   The user confirms, then `schedule_apply`:
   - reads the XML as it is on disk now (UTF-8, BOM dropped);
   - calls `fm_schedule::merge` with the schedule shown in the preview;
   - on success, copies the original bytes to the first free name of
     `<file>.bak`, `<file>.bak.1`, … (never replacing an existing file);
   - replaces the XML atomically (`fm_core::atomic_write`).

   Nothing is written when `merge` returns an error.

## Where the work goes

| What | Where |
| --- | --- |
| The parsed data model | `Schedule` in `crates/fm-schedule/src/model.rs` (placeholder: columns + rows) |
| The preview table | `Schedule::preview` → `SchedulePreview { columns, rows }` |
| Parsing | `parse(text) -> Result<Schedule, ParseError>` in `src/parse.rs` |
| Updating the XML | `merge(schedule, xml) -> Result<MergeOutcome, MergeError>` in `src/merge.rs` |

- `ParseError::Invalid { line, message }`: `message` is shown to the user as
  "Line N of the schedule file: message".
- `MergeOutcome { xml, added }`: `xml` is the whole new document; `added` is
  the count in the success message.
- `MergeError::InvalidXml` / `Conflict`: shown as "could not be added",
  with the message under Details.
- `merge` must not touch the filesystem.

Changing `Schedule`'s fields needs no UI change as long as `preview` keeps
returning columns and rows. Changing `SchedulePreview` or any type in
`src-tauri/src/tools/schedule/types.rs` is a boundary change: regenerate
`lib/generated/` and update the view.

The shell tests (`src-tauri/src/tools/schedule/tests.rs`) drive the whole flow
with fake hooks, so they keep passing whatever the real hooks do. Add the
parser's and merger's own tests in `fm-schedule`, and replace its
`stub_reports_not_implemented` tests when the stubs go.
