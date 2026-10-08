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

/// A core with `text` and `xml` written and chosen.
fn ready(d: &TempDir, text: &str, xml: &str) -> (Core, PathBuf) {
    let mut c = core();
    c.set_text(Some(&write(d, "s.txt", text)));
    let xml = write(d, "s.xml", xml);
    c.set_xml(Some(&xml));
    (c, xml)
}

#[test]
fn empty_session() {
    let s = Core::default().snapshot();
    assert_eq!(s.text, None);
    assert_eq!(s.xml, None);
    assert!(!s.can_apply);
}

#[test]
fn setting_files_does_not_parse() {
    let d = dir();
    // The fake parser rejects `!`; choosing the file must not run it.
    let (c, _) = ready(&d, "a!\n", "<schedule></schedule>");
    let s = c.snapshot();
    assert_eq!(s.text.as_ref().unwrap().error, None);
    assert!(s.can_apply);
}

#[test]
fn a_bad_path_is_kept_with_its_error() {
    let d = dir();
    let mut c = core();
    let gone = d.path().join("gone.txt");
    c.set_text(Some(&gone));
    let text = c.snapshot().text.unwrap();
    assert_eq!(text.path, gone.to_string_lossy());
    assert_eq!(text.error.map(|e| e.kind), Some(K::NotAFile));

    c.set_xml(Some(d.path()));
    assert_eq!(
        c.snapshot().xml.unwrap().error.map(|e| e.kind),
        Some(K::NotAFile)
    );

    let utf16 = d.path().join("utf16.xml");
    fs::write(&utf16, [0xff, 0xfe, 0x3c, 0x00]).unwrap();
    c.set_xml(Some(&utf16));
    assert_eq!(
        c.snapshot().xml.unwrap().error.map(|e| e.kind),
        Some(K::XmlUnreadable)
    );

    let latin1 = d.path().join("latin1.txt");
    fs::write(&latin1, [0x61, 0xe9, 0x0a]).unwrap();
    c.set_text(Some(&latin1));
    assert_eq!(
        c.snapshot().text.unwrap().error.map(|e| e.kind),
        Some(K::TextUnreadable)
    );
    assert!(!c.snapshot().can_apply);
    assert_eq!(c.apply().unwrap_err().kind, K::TextUnreadable);
}

#[test]
fn none_clears_a_slot() {
    let d = dir();
    let (mut c, _) = ready(&d, "a\n", "<schedule></schedule>");
    c.set_text(None);
    assert_eq!(c.snapshot().text, None);
    assert_eq!(c.apply().unwrap_err().kind, K::NoText);
    c.set_xml(None);
    assert_eq!(c.snapshot().xml, None);
}

#[test]
fn drop_sorts_xml_from_text() {
    let d = dir();
    let mut c = core();
    let text = write(&d, "week.TXT", "a\n");
    let xml = write(&d, "plan.XML", "<schedule></schedule>");
    c.set_dropped(&[xml.clone(), text.clone()]).unwrap();
    let s = c.snapshot();
    assert_eq!(s.text.unwrap().path, text.to_string_lossy());
    assert_eq!(s.xml.unwrap().path, xml.to_string_lossy());

    // A drop of one kind keeps the other slot.
    let other = write(&d, "other.csv", "b\n");
    c.set_dropped(std::slice::from_ref(&other)).unwrap();
    let s = c.snapshot();
    assert_eq!(s.text.unwrap().path, other.to_string_lossy());
    assert_eq!(s.xml.unwrap().path, xml.to_string_lossy());
}

#[test]
fn ambiguous_drop_changes_nothing() {
    let d = dir();
    let mut c = core();
    let a = write(&d, "a.xml", "<a/>");
    let b = write(&d, "b.xml", "<b/>");
    let t = write(&d, "t.txt", "t");
    assert_eq!(
        c.set_dropped(&[a, b, t.clone()]).unwrap_err().kind,
        K::DropAmbiguous
    );
    let u = write(&d, "u.txt", "u");
    assert_eq!(c.set_dropped(&[t, u]).unwrap_err().kind, K::DropAmbiguous);
    assert_eq!(c.snapshot().text, None);
    assert_eq!(c.snapshot().xml, None);
}

#[test]
fn default_hooks_are_the_stubs() {
    let d = dir();
    let mut c = Core::default();
    c.set_text(Some(&write(&d, "s.txt", "a\n")));
    let xml = write(&d, "s.xml", "<schedule/>");
    c.set_xml(Some(&xml));
    assert_eq!(c.apply().unwrap_err().kind, K::ParseNotImplemented);
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 2);
}

#[test]
fn parse_runs_on_apply_and_its_error_names_the_line() {
    let d = dir();
    let (c, xml) = ready(&d, "a\nb!\n", "<schedule></schedule>");
    let e = c.apply().unwrap_err();
    assert_eq!(e.kind, K::ParseFailed);
    assert_eq!(e.line, Some(2));
    assert_eq!(fs::read_to_string(&xml).unwrap(), "<schedule></schedule>");
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 2);
}

#[test]
fn apply_reads_the_text_as_it_is_now() {
    let d = dir();
    let (c, xml) = ready(&d, "a\n", "<schedule></schedule>");
    fs::write(d.path().join("s.txt"), "\u{feff}b\r\nc\r\n").unwrap();
    assert_eq!(c.apply().unwrap().added, 2);
    assert_eq!(
        fs::read_to_string(&xml).unwrap(),
        "<schedule><item>b</item><item>c</item></schedule>"
    );
}

#[test]
fn apply_backs_up_then_writes_the_merged_xml() {
    let d = dir();
    let (c, xml) = ready(&d, "a\nb\n", "<schedule></schedule>");

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
    let (c, xml) = ready(&d, "a\n", "<other/>");
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
    c.set_text(Some(&write(&d, "s.txt", "a\n")));
    c.set_xml(Some(&write(&d, "s.xml", "<schedule/>")));
    assert_eq!(c.apply().unwrap_err().kind, K::MergeNotImplemented);
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 2);
}

#[test]
fn files_removed_after_choosing_are_reported() {
    let d = dir();
    let (c, xml) = ready(&d, "a\n", "<schedule></schedule>");
    fs::remove_file(&xml).unwrap();
    assert_eq!(c.apply().unwrap_err().kind, K::XmlUnreadable);
    fs::remove_file(d.path().join("s.txt")).unwrap();
    assert_eq!(c.apply().unwrap_err().kind, K::TextUnreadable);
}

#[test]
fn xml_bom_is_kept_and_backed_up() {
    let d = dir();
    let (c, xml) = ready(&d, "a\n", "\u{feff}<schedule></schedule>");
    let summary = c.apply().unwrap();
    assert_eq!(
        fs::read_to_string(&xml).unwrap(),
        "\u{feff}<schedule><item>a</item></schedule>"
    );
    assert_eq!(
        fs::read_to_string(&summary.backup_path).unwrap(),
        "\u{feff}<schedule></schedule>"
    );
}
