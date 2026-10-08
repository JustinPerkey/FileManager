use std::fs;
use std::path::PathBuf;

use fm_schedule::{MergeError, MergeOutcome, ParseError, Schedule};
use tempfile::TempDir;

use super::core::{Core, Hooks};
use super::types::ScheduleErrorKind as K;

fn dir() -> TempDir {
    tempfile::tempdir().expect("tempdir")
}

fn write(dir: &TempDir, name: &str, text: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, text).unwrap();
    path
}

/// One row per non-empty line; a line containing `!` is an error.
fn fake_parse(text: &str) -> Result<Schedule, ParseError> {
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.contains('!') {
            return Err(ParseError::Invalid {
                line: Some(i as u32 + 1),
                message: "bad line".into(),
            });
        }
        if !line.is_empty() {
            rows.push(vec![line.to_owned()]);
        }
    }
    Ok(Schedule {
        columns: vec!["Item".into()],
        rows,
    })
}

/// Appends each row as `<item>` before `</schedule>`.
fn fake_merge(schedule: &Schedule, xml: &str) -> Result<MergeOutcome, MergeError> {
    let at = xml
        .rfind("</schedule>")
        .ok_or_else(|| MergeError::InvalidXml("no </schedule>".into()))?;
    let items: String = schedule
        .rows
        .iter()
        .map(|r| format!("<item>{}</item>", r[0]))
        .collect();
    Ok(MergeOutcome {
        xml: format!("{}{items}{}", &xml[..at], &xml[at..]),
        added: schedule.rows.len() as u32,
    })
}

fn core() -> Core {
    Core::with_hooks(Hooks {
        parse: fake_parse,
        merge: fake_merge,
    })
}

#[test]
fn empty_session() {
    let s = Core::default().snapshot();
    assert_eq!(s.text_path, None);
    assert_eq!(s.xml_path, None);
    assert_eq!(s.preview, None);
    assert_eq!(s.parse_error, None);
    assert!(!s.can_apply);
}

#[test]
fn default_hooks_are_the_stubs() {
    let d = dir();
    let mut c = Core::default();
    c.open_text(&write(&d, "s.txt", "a\n")).unwrap();
    let s = c.snapshot();
    assert_eq!(s.parse_error.map(|e| e.kind), Some(K::ParseNotImplemented));
    assert!(!s.can_apply);
}

#[test]
fn open_text_previews_and_strips_bom() {
    let d = dir();
    let mut c = core();
    c.open_text(&write(&d, "s.txt", "\u{feff}a\r\nb\r\n"))
        .unwrap();
    let preview = c.snapshot().preview.expect("preview");
    assert_eq!(preview.columns, ["Item"]);
    assert_eq!(preview.rows, [["a"], ["b"]]);
}

#[test]
fn parse_failure_is_kept_in_the_session_with_its_line() {
    let d = dir();
    let mut c = core();
    c.open_text(&write(&d, "s.txt", "a\nb!\n")).unwrap();
    let s = c.snapshot();
    let e = s.parse_error.expect("error");
    assert_eq!(e.kind, K::ParseFailed);
    assert_eq!(e.line, Some(2));
    assert!(s.text_path.is_some());
    assert_eq!(s.preview, None);
}

#[test]
fn unreadable_or_missing_text_leaves_the_session_unchanged() {
    let d = dir();
    let mut c = core();
    let good = write(&d, "s.txt", "a\n");
    c.open_text(&good).unwrap();

    let bad = d.path().join("latin1.txt");
    fs::write(&bad, [0x61, 0xe9, 0x0a]).unwrap();
    assert_eq!(c.open_text(&bad).unwrap_err().kind, K::TextUnreadable);
    assert_eq!(
        c.open_text(&d.path().join("gone.txt")).unwrap_err().kind,
        K::NotAFile
    );
    assert_eq!(c.open_text(d.path()).unwrap_err().kind, K::NotAFile);
    assert_eq!(
        c.snapshot().text_path,
        Some(good.to_string_lossy().into_owned())
    );
}

#[test]
fn reload_rereads_the_file() {
    let d = dir();
    let mut c = core();
    assert_eq!(c.reload_text().unwrap_err().kind, K::NoText);
    let path = write(&d, "s.txt", "a\n");
    c.open_text(&path).unwrap();
    fs::write(&path, "a\nb\n").unwrap();
    c.reload_text().unwrap();
    assert_eq!(c.snapshot().preview.unwrap().rows.len(), 2);
}

#[test]
fn apply_needs_a_parsed_text_and_an_xml() {
    let d = dir();
    let mut c = core();
    assert_eq!(c.apply().unwrap_err().kind, K::NoText);
    c.open_text(&write(&d, "s.txt", "a!\n")).unwrap();
    assert_eq!(c.apply().unwrap_err().kind, K::ParseFailed);
    c.open_text(&write(&d, "s.txt", "a\n")).unwrap();
    assert_eq!(c.apply().unwrap_err().kind, K::NoXml);
    assert!(!c.snapshot().can_apply);
    c.open_xml(&write(&d, "s.xml", "<schedule></schedule>"))
        .unwrap();
    assert!(c.snapshot().can_apply);
}

#[test]
fn apply_backs_up_then_writes_the_merged_xml() {
    let d = dir();
    let mut c = core();
    c.open_text(&write(&d, "s.txt", "a\nb\n")).unwrap();
    let xml = write(&d, "s.xml", "<schedule></schedule>");
    c.open_xml(&xml).unwrap();

    let first = c.apply().unwrap();
    assert_eq!(first.added, 2);
    assert_eq!(
        fs::read_to_string(&xml).unwrap(),
        "<schedule><item>a</item><item>b</item></schedule>"
    );
    assert_eq!(
        PathBuf::from(&first.backup_path),
        d.path().join("s.xml.bak")
    );
    assert_eq!(
        fs::read_to_string(&first.backup_path).unwrap(),
        "<schedule></schedule>"
    );

    // A second apply never replaces the first backup.
    let second = c.apply().unwrap();
    assert_eq!(
        PathBuf::from(&second.backup_path),
        d.path().join("s.xml.bak.1")
    );
    assert_eq!(
        fs::read_to_string(&first.backup_path).unwrap(),
        "<schedule></schedule>"
    );
    assert_eq!(
        fs::read_to_string(&second.backup_path).unwrap(),
        "<schedule><item>a</item><item>b</item></schedule>"
    );
}

#[test]
fn failed_merge_writes_nothing() {
    let d = dir();
    let mut c = core();
    c.open_text(&write(&d, "s.txt", "a\n")).unwrap();
    let xml = write(&d, "s.xml", "<other/>");
    c.open_xml(&xml).unwrap();
    assert_eq!(c.apply().unwrap_err().kind, K::MergeFailed);
    assert_eq!(fs::read_to_string(&xml).unwrap(), "<other/>");
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 2);
}

#[test]
fn stub_merge_writes_nothing() {
    let d = dir();
    let mut c = Core::with_hooks(Hooks {
        parse: fake_parse,
        ..Hooks::default()
    });
    c.open_text(&write(&d, "s.txt", "a\n")).unwrap();
    let xml = write(&d, "s.xml", "<schedule/>");
    c.open_xml(&xml).unwrap();
    assert_eq!(c.apply().unwrap_err().kind, K::MergeNotImplemented);
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 2);
}

#[test]
fn xml_removed_after_choosing_is_reported() {
    let d = dir();
    let mut c = core();
    c.open_text(&write(&d, "s.txt", "a\n")).unwrap();
    let xml = write(&d, "s.xml", "<schedule></schedule>");
    c.open_xml(&xml).unwrap();
    fs::remove_file(&xml).unwrap();
    assert_eq!(c.apply().unwrap_err().kind, K::XmlUnreadable);
}
