use super::*;
use crate::format::ArchiveFormat;

/// A manifest with one file whose extra lines are `extra`.
fn one(extra: &str) -> String {
    format!(
        "version = 1\nname = \"t\"\n[[file]]\nid = \"a\"\nsource = \"a\"\ndir = \"/opt\"\n{extra}"
    )
}

fn with_dir(dir: &str) -> String {
    format!("version = 1\nname = \"t\"\n[[file]]\nid = \"a\"\nsource = \"a\"\ndir = \"{dir}\"\n")
}

fn errors(text: &str) -> Vec<Diagnostic> {
    match parse(text) {
        Ok((_, w)) => panic!("expected errors, got ok with {w:?}"),
        Err(d) => d
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect(),
    }
}

fn ok(text: &str) -> (Manifest, Vec<Diagnostic>) {
    match parse(text) {
        Ok(v) => v,
        Err(d) => panic!("expected ok, got {d:?}"),
    }
}

fn assert_err(text: &str, needle: &str) -> Diagnostic {
    let errs = errors(text);
    let hit = errs.iter().find(|d| d.message.contains(needle));
    let hit = hit.unwrap_or_else(|| panic!("no error containing {needle:?} in {errs:?}"));
    assert!(hit.line >= 1 && hit.col >= 1);
    hit.clone()
}

#[test]
fn rejects_relative_dir() {
    assert_ok_dir("/opt/x");
    assert_ok_dir("/");
    assert_ok_dir("/opt/x/");
    let d = assert_err(&with_dir("opt/x"), "absolute");
    assert_eq!(d.entry_id.as_deref(), Some("a"));
    assert!(d.message.contains("`a`"));
    assert_eq!(d.line, 6);
    assert_err(&with_dir(""), "absolute");
}

fn assert_ok_dir(dir: &str) {
    ok(&with_dir(dir));
}

#[test]
fn rejects_dotdot() {
    for d in ["/opt/../etc", "/..", "/opt/.."] {
        assert_err(&with_dir(d), "..");
    }
    ok(&with_dir("/opt/..x"));
}

#[test]
fn rejects_dot_segment_in_dir() {
    for d in ["/opt/./x", "/opt/.", "/.", "/./opt"] {
        assert_err(&with_dir(d), "`.` segment");
    }
    for d in ["/opt/.x", "/opt/x.", "/opt/..x"] {
        ok(&with_dir(d));
    }
}

#[test]
fn rejects_backslash_in_dir() {
    assert_err(&with_dir("/opt\\\\x"), "backslash");
    assert_err(&with_dir("/opt/\\u0000x"), "NUL");
    assert_err(&with_dir("/opt//x"), "empty segment");
    assert_err(&with_dir("/opt/x//"), "empty segment");
}

#[test]
fn rejects_duplicate_id() {
    let t = "version = 1\nname = \"t\"\n\
        [[file]]\nid = \"a\"\nsource = \"a\"\ndir = \"/x\"\n\
        [[file]]\nid = \"a\"\nsource = \"b\"\ndir = \"/x\"\n";
    let d = assert_err(t, "duplicate id");
    assert_eq!(d.line, 8);
    assert!(d.message.contains("line 4"));
    assert_err(
        &one("").replace("id = \"a\"", "id = \"\""),
        "id must not be empty",
    );
}

#[test]
fn rejects_duplicate_target() {
    let t = "version = 1\nname = \"t\"\n\
        [[file]]\nid = \"a\"\nsource = \"a\"\ndir = \"/x/\"\n\
        [[file]]\nid = \"b\"\nsource = \"z\"\nname = \"a\"\ndir = \"/x\"\n";
    let d = assert_err(t, "/x/a");
    assert_eq!(d.entry_id.as_deref(), Some("b"));
    // Located at the `name` value when `name` is set.
    assert_eq!((d.line, d.col), (10, 8));
    // Otherwise at the `dir` value.
    let t2 = "version = 1\nname = \"t\"\n\
        [[file]]\nid = \"a\"\nsource = \"a\"\ndir = \"/x/\"\n\
        [[file]]\nid = \"b\"\nsource = \"a\"\ndir = \"/x\"\n";
    let d = assert_err(t2, "/x/a");
    assert_eq!((d.line, d.col), (10, 7));
    // Case-sensitive: different case is a different target.
    let t = t.replace("name = \"a\"", "name = \"A\"");
    ok(&t);
}

#[test]
fn rejects_target_that_is_another_entrys_directory() {
    let a = "[[file]]\nid = \"a\"\nsource = \"s\"\nname = \"gateway\"\ndir = \"/opt\"\n";
    let head = "version = 1\nname = \"t\"\n";
    let b = |dir: &str| format!("[[file]]\nid = \"b\"\nsource = \"t\"\ndir = \"{dir}\"\n");
    for bdir in ["/opt/gateway/bin", "/opt/gateway", "/opt/gateway/"] {
        for order in [0, 1] {
            let body = if order == 0 {
                format!("{a}{}", b(bdir))
            } else {
                format!("{}{a}", b(bdir))
            };
            let errs = errors(&format!("{head}{body}"));
            assert_eq!(errs.len(), 1, "{bdir} {order}: {errs:?}");
            assert_eq!(errs[0].entry_id.as_deref(), Some("a"));
            assert!(errs[0].message.contains("`/opt/gateway`"));
            assert!(errs[0].message.contains("file `b`"), "{}", errs[0].message);
        }
    }
    ok(&format!("{head}{a}{}", b("/opt/gatewayx/bin")));
}

#[test]
fn rejects_unknown_key_with_location() {
    let t = one("colour = \"red\"\n");
    let d = errors(&t).remove(0);
    assert!(d.message.contains("colour"), "{}", d.message);
    assert_eq!((d.line, d.col), (7, 1));
    assert_eq!(errors("version = 1\nname = \"t\"\nbogus = 1\n").len(), 1);
    let d = &errors("version = 1\nname = \"t\"\n[defaults]\nmod = \"0644\"\n")[0];
    assert!(d.message.contains("`mod`"), "{}", d.message);
    assert_eq!((d.line, d.col), (4, 1));
}

#[test]
fn file_table_errors_name_the_entry_id() {
    let head = "version = 1\nname = \"t\"\n[[file]]\nid = \"a\"\nsource = \"a\"\n";
    // (extra lines, expected line)
    for (extra, line) in [
        ("dir = \"/x\"\ncolour = \"red\"\n", 7),
        ("dir = \"/x\"\nmode = 644\n", 7),
        ("", 3),
    ] {
        let t = format!("{head}{extra}");
        let errs = errors(&t);
        assert_eq!(errs.len(), 1, "{t}: {errs:?}");
        assert_eq!(errs[0].entry_id.as_deref(), Some("a"), "{errs:?}");
        assert!(
            errs[0].message.starts_with("file `a`: "),
            "{}",
            errs[0].message
        );
        assert_eq!(errs[0].line, line, "{errs:?}");
    }
    let d = errors("version = 1\nname = \"t\"\n[[file]]\nid = 3\nsource = \"a\"\ndir = \"/x\"\n")
        .remove(0);
    assert_eq!(d.entry_id, None);
    assert!(d.message.starts_with("[[file]] #1: "), "{}", d.message);
}

#[test]
fn unknown_keys_are_reported_with_validation_errors() {
    let t = "version = 2\nname = \"t\"\nbogus = 1\n[defaults]\nmod = \"0644\"\n\
        [[file]]\nid = \"a\"\nsource = \"a\"\ndir = \"/x\"\ncolour = 1\n\
        [[file]]\nid = \"b\"\nsource = \"b\"\ndir = \"rel\"\n";
    let errs = errors(t);
    assert_eq!(errs.len(), 5, "{errs:?}");
    let lines: Vec<u32> = errs.iter().map(|d| d.line).collect();
    assert_eq!(lines, [1, 3, 5, 10, 14]);
}

#[test]
fn rejects_empty_manifest_name() {
    assert_err("version = 1\nname = \"\"\n", "name must not be empty");
    ok("version = 1\nname = \"a/b\"\n");
    ok("version = 1\nname = \".\"\n");
}

#[test]
fn rejects_mode_out_of_range() {
    for m in ["17777", "0888", "rwx", ""] {
        assert_err(&one(&format!("mode = \"{m}\"")), "mode");
    }
    let t = "version = 1\nname = \"t\"\n[defaults]\ndir_mode = \"12345\"\n";
    assert_err(t, "defaults.dir_mode");
    // Wrong type is a located error too.
    let d = errors(&one("mode = 644")).remove(0);
    assert_eq!(d.line, 7);
    ok(&one("mode = \"7777\""));
}

#[test]
fn defaults_apply_and_per_file_overrides_win() {
    let t = "version = 1\nname = \"t\"\n\
        [defaults]\nmode = \"0600\"\ndir_mode = \"0700\"\nuid = 5\ngid = 6\nuname = \"u\"\ngname = \"g\"\n\
        [[file]]\nid = \"a\"\nsource = \"a.txt\"\ndir = \"/x\"\n\
        [[file]]\nid = \"b\"\nsource = \"b.txt\"\nname = \"renamed\"\ndir = \"/y/\"\n\
        mode = \"0755\"\nuid = 1\ngid = 2\nuname = \"bu\"\ngname = \"bg\"\n";
    let (m, w) = ok(t);
    assert!(w.is_empty());
    assert_eq!(m.dir_mode(), 0o700);
    assert_eq!(m.default_owner().uid, 5);
    let a = &m.entries()[0];
    assert_eq!(
        (a.mode(), a.uid(), a.gid(), a.uname(), a.gname()),
        (0o600, 5, 6, "u", "g")
    );
    assert_eq!(a.target_path(), "/x/a.txt");
    assert!(!a.normalize_eol());
    let b = &m.entries()[1];
    assert_eq!(
        (b.mode(), b.uid(), b.gid(), b.uname(), b.gname()),
        (0o755, 1, 2, "bu", "bg")
    );
    assert_eq!(b.target_path(), "/y/renamed");
    assert_eq!(b.source(), "b.txt");

    let (m, _) = ok(&one(""));
    let e = &m.entries()[0];
    assert_eq!(
        (e.mode(), e.uid(), e.gid(), e.uname(), e.gname()),
        (0o644, 0, 0, "root", "root")
    );
    assert_eq!(m.dir_mode(), 0o755);

    assert_err(&one("uid = 4294967296"), "u32");
    assert_err(&one("gid = -1"), "u32");
    assert_err(&one("uname = \"\""), "uname");
    assert_err(&one(&format!("gname = \"{}\"", "g".repeat(33))), "gname");
    ok(&one(&format!("gname = \"{}\"", "g".repeat(32))));
}

#[test]
fn warns_on_shared_source_name() {
    let t = "version = 1\nname = \"t\"\n\
        [[file]]\nid = \"a\"\nsource = \"Cfg.txt\"\ndir = \"/x\"\n\
        [[file]]\nid = \"b\"\nsource = \"cfg.TXT\"\ndir = \"/y\"\n";
    let (_, w) = ok(t);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].severity, Severity::Warning);
    assert_eq!(w[0].entry_id.as_deref(), Some("b"));
    assert!(w[0].message.contains("`a`"));
    let (_, w) = ok(&t.replace("cfg.TXT", "other"));
    assert!(w.is_empty());
}

#[test]
fn warns_on_long_target() {
    let (_, w) = ok(&with_dir(&format!("/{}", "d".repeat(150))));
    assert_eq!(w.len(), 1);
    assert!(w[0].message.contains("100 bytes or longer"));
    assert!(w[0].message.contains("153 bytes"));
    let (_, w) = ok(&with_dir("/opt"));
    assert!(w.is_empty());
}

#[test]
fn long_name_warning_counts_leading_slash() {
    // Stored path is "/" + dir + "/" + name; name is "a" (1 byte).
    let make = |total: usize| with_dir(&format!("/{}", "d".repeat(total - 3)));
    let (_, w) = ok(&make(100));
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].message.contains("(100 bytes)"));
    let (_, w) = ok(&make(99));
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn warns_on_no_files() {
    let (m, w) = ok("version = 1\nname = \"t\"\n");
    assert!(m.entries().is_empty());
    assert_eq!(w.len(), 1);
    assert!(w[0].message.contains("no [[file]]"));
}

#[test]
fn collects_all_errors() {
    let t = "version = 2\nname = \"\"\noutput_name = \"a/b\"\n\
        [defaults]\nmode = \"9\"\n\
        [[file]]\nid = \"a\"\nsource = \"..\"\ndir = \"rel\"\n\
        [[file]]\nid = \"a\"\nsource = \"s\"\ndir = \"/x\"\nuid = -3\n\
        [[file]]\nid = \"c\"\nsource = \"c\"\ndir = \"/x\"\ncolour = 1\nmode = 644\n";
    let errs = errors(t);
    assert!(errs.len() >= 9, "{errs:?}");
    let lines: Vec<u32> = errs.iter().map(|d| d.line).collect();
    let mut sorted = lines.clone();
    sorted.sort_unstable();
    assert_eq!(lines, sorted);
    for needle in [
        "version",
        "name must",
        "output_name",
        "defaults.mode",
        "source",
        "absolute",
        "duplicate id",
        "uid",
        "unknown key `colour`",
        "invalid type",
    ] {
        assert!(errs.iter().any(|d| d.message.contains(needle)), "{needle}");
    }
}

#[test]
fn rejects_type_and_missing_field_errors() {
    for t in [
        "name = \"t\"\n",
        "version = 1\n",
        "version = \"1\"\nname = \"t\"\n",
        "version = 1\nname = \"t\"\n[[file]]\nid = \"a\"\nsource = \"a\"\n",
        "version = 1\nname = \"t\"\n[[file]]\nid = 3\nsource = \"a\"\ndir = \"/x\"\n",
        "not toml at all [",
    ] {
        let d = errors(t);
        assert_eq!(d.len(), 1, "{t}");
        assert!(d[0].line >= 1);
        if t.contains("[[file]]") && !t.contains("id = 3") {
            assert_eq!(d[0].entry_id.as_deref(), Some("a"), "{t}");
        }
    }
    let d = errors("version = 1\nname = \"t\"\n[[file]]\nid = \"a\"\nsource = \"a\"\n");
    assert!(d[0].message.contains("dir"), "{}", d[0].message);
}

#[test]
fn rejects_bad_names() {
    for bad in ["", ".", "..", "a/b", "a\\\\b", "a\\u0000b"] {
        let t = one("").replace("source = \"a\"", &format!("source = \"{bad}\""));
        assert_err(&t, "source");
        assert_err(&one(&format!("name = \"{bad}\"")), "name");
    }
    ok(&one("name = \"...\""));
}

#[test]
fn rejects_output_name_with_separator() {
    for bad in ["a/b.tar", "a\\\\b.tar", "", "a\\u0000.tar"] {
        let t = format!("version = 1\nname = \"t\"\noutput_name = \"{bad}\"\n");
        assert_err(&t, "output_name");
    }
    let (m, _) = ok("version = 1\nname = \"t\"\noutput_name = \"a.tar\"\n");
    assert_eq!(m.output_name(), Some("a.tar"));
}

#[test]
fn normalize_eol_defaults_false_and_parses_per_file() {
    let (m, _) = ok(&one(""));
    assert!(!m.entries()[0].normalize_eol());
    let (m, _) = ok(&one("normalize_eol = true"));
    assert!(m.entries()[0].normalize_eol());
    let (m, _) = ok(&one("normalize_eol = false"));
    assert!(!m.entries()[0].normalize_eol());
    let d = errors(&one("normalize_eol = \"yes\"")).remove(0);
    assert_eq!(d.line, 7);
    let v = ManifestView::from(&ok(&one("normalize_eol = true")).0);
    assert!(v.entries[0].normalize_eol);
}

#[test]
fn normalize_eol_rejected_in_defaults() {
    let t = "version = 1\nname = \"t\"\n[defaults]\nnormalize_eol = true\n";
    let d = errors(t).remove(0);
    assert!(d.message.contains("normalize_eol"), "{}", d.message);
    assert_eq!((d.line, d.col), (4, 1));
}

#[test]
fn default_format_follows_output_name_suffix() {
    let f = |o: &str| {
        let t = if o.is_empty() {
            "version = 1\nname = \"t\"\n".to_string()
        } else {
            format!("version = 1\nname = \"t\"\noutput_name = \"{o}\"\n")
        };
        ok(&t).0.default_format()
    };
    assert_eq!(f(""), ArchiveFormat::Tar);
    assert_eq!(f("x.zip"), ArchiveFormat::Tar);
    assert_eq!(f("x.tar.zst"), ArchiveFormat::TarZst);
    assert_eq!(f("x.TGZ"), ArchiveFormat::TarGz);
    assert_eq!(f("x.tar.xz"), ArchiveFormat::TarXz);
}

#[test]
fn view_renders_mode_and_target() {
    let (m, _) = ok(&one("mode = \"0755\""));
    let v = ManifestView::from(&m);
    assert_eq!(v.entries[0].mode, "0755");
    assert_eq!(v.entries[0].mode_text, "rwxr-xr-x");
    assert_eq!(v.entries[0].owner, "root:root");
    assert_eq!(v.entries[0].target_path, "/opt/a");
    let (m, _) = ok(&one("gname = \"gateway\"\ngid = 990"));
    let v = ManifestView::from(&m);
    assert_eq!(v.entries[0].owner, "root:gateway");
    assert_eq!(v.entries[0].gid, 990);
}

#[test]
fn example_manifest_is_valid() {
    let (m, warnings) = parse(include_str!("../../../../examples/tarpack/example.toml"))
        .unwrap_or_else(|d| panic!("example is invalid: {d:?}"));
    assert!(warnings.is_empty(), "{warnings:?}");
    let m = &m;
    assert_eq!(m.name(), "Gateway deploy");
    assert_eq!(m.default_format(), ArchiveFormat::TarZst);
    assert_eq!(m.entries().len(), 3);
    assert_eq!(m.entries()[1].gid(), 990);
    assert_eq!(m.entries()[1].gname(), "gateway");
    assert!(m.entries()[2].normalize_eol());
}

#[test]
fn load_hashes_exact_bytes() {
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("m.toml");
    let bytes = b"\xef\xbb\xbfversion = 1\r\nname = \"t\"\r\n";
    std::fs::write(&p, bytes).unwrap();
    let loaded = load(&p).unwrap();
    let want: [u8; 32] = Sha256::digest(bytes).into();
    assert_eq!(loaded.sha256, want);
    assert_eq!(loaded.path, p);

    std::fs::write(&p, b"version = 1\nname = \"t\"\n").unwrap();
    assert_ne!(load(&p).unwrap().sha256, want);

    std::fs::write(&p, b"\xff\xfe").unwrap();
    assert!(matches!(load(&p), Err(LoadError::Invalid(_))));
    assert!(matches!(
        load(&dir.path().join("missing.toml")),
        Err(LoadError::Io(_))
    ));
}
