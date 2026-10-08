use std::fs;
use std::io::{BufReader, Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::encode::Decoder;
use super::eol::{converted_len, CrlfToLf};
use super::write::{temp_prefix, write_archive_with, Hooks};
use super::*;
use crate::format::ArchiveFormat;
use crate::manifest::{parse, ParseReport};
use crate::sources::Assignments;

const ALL: [ArchiveFormat; 4] = ArchiveFormat::ALL;

fn ext(f: ArchiveFormat) -> &'static str {
    f.extension()
}

/// A manifest of `(id, dir, extra)` entries; each `source` is `<id>.bin`.
fn manifest(header: &str, entries: &[(&str, &str, &str)]) -> String {
    let mut s = format!("version = 1\n{header}\n");
    for (id, dir, extra) in entries {
        s += &format!(
            "[[file]]\nid = \"{id}\"\nsource = \"{id}.bin\"\nname = \"{id}\"\ndir = \"{dir}\"\n{extra}\n"
        );
    }
    s
}

struct Fx {
    tmp: TempDir,
    report: ParseReport,
    assign: Assignments,
}

impl Fx {
    /// Writes `<id>.bin` for each of `files` and assigns exactly those ids.
    fn new(text: &str, files: &[(&str, &[u8])]) -> Fx {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("src")).unwrap();
        let mut assign = Assignments::new();
        for (id, bytes) in files {
            let p = tmp.path().join("src").join(format!("{id}.bin"));
            fs::write(&p, bytes).unwrap();
            assign.insert(*id, p);
        }
        Fx {
            tmp,
            report: parse(text),
            assign,
        }
    }

    fn valid(text: &str, files: &[(&str, &[u8])]) -> Fx {
        let fx = Fx::new(text, files);
        assert!(
            fx.report.is_valid(),
            "{:?} {:?}",
            fx.report.errors,
            fx.report.failures
        );
        fx
    }

    fn plan(&self) -> ArchivePlan {
        ArchivePlan::new(&self.report, &self.assign).unwrap()
    }

    fn out_dir(&self) -> PathBuf {
        let d = self.tmp.path().join("out");
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn build(&self, format: ArchiveFormat) -> (PathBuf, BuildSummary) {
        let out = self.out_dir().join(format!("a{}", ext(format)));
        let s = write_archive(&self.plan(), &out, format, false, |_| {}).unwrap();
        (out, s)
    }

    fn build_with(
        &self,
        format: ArchiveFormat,
        overwrite: bool,
        hooks: &Hooks,
    ) -> (PathBuf, Result<BuildSummary, BuildError>) {
        let out = self.out_dir().join(format!("a{}", ext(format)));
        let r = write_archive_with(&self.plan(), &out, format, overwrite, &mut |_| {}, hooks);
        (out, r)
    }
}

fn dir_listing(p: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(p)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

struct Rec {
    name: Vec<u8>,
    dir: bool,
    mode: u32,
    uid: u64,
    gid: u64,
    uname: String,
    gname: String,
    mtime: u64,
    data: Vec<u8>,
}

fn decompressed(path: &Path, format: ArchiveFormat) -> Vec<u8> {
    let f = BufReader::new(fs::File::open(path).unwrap());
    let mut d = Decoder::new(format, f).unwrap();
    let mut v = Vec::new();
    d.read_to_end(&mut v).unwrap();
    v
}

fn read_back(path: &Path, format: ArchiveFormat) -> Vec<Rec> {
    let raw = decompressed(path, format);
    let mut ar = tar::Archive::new(Cursor::new(raw));
    let mut out = Vec::new();
    for e in ar.entries().unwrap() {
        let mut e = e.unwrap();
        let mut data = Vec::new();
        e.read_to_end(&mut data).unwrap();
        let h = e.header();
        out.push(Rec {
            name: e.path_bytes().into_owned(),
            dir: h.entry_type() == tar::EntryType::Directory,
            mode: h.mode().unwrap() & 0o7777,
            uid: h.uid().unwrap(),
            gid: h.gid().unwrap(),
            uname: String::from_utf8_lossy(h.username_bytes().unwrap()).into_owned(),
            gname: String::from_utf8_lossy(h.groupname_bytes().unwrap()).into_owned(),
            mtime: h.mtime().unwrap(),
            data,
        });
    }
    out
}

fn names(recs: &[Rec]) -> Vec<String> {
    recs.iter()
        .map(|r| String::from_utf8(r.name.clone()).unwrap())
        .collect()
}

const HDR: &str = "name = \"t\"\n[defaults]\nmode = \"0644\"\ndir_mode = \"0750\"\nuid = 7\ngid = 8\nuname = \"svc\"\ngname = \"grp\"";

fn simple() -> Fx {
    Fx::valid(
        &manifest(
            HDR,
            &[
                ("a", "/opt/gw/bin", "mode = \"0755\""),
                ("b", "/opt/gw/etc", "uid = 1000\nuname = \"bob\""),
            ],
        ),
        &[("a", b"alpha bytes\r\n"), ("b", b"beta")],
    )
}

// ---- plan ----

fn failing_fixture() -> Fx {
    let text = format!(
        "version = 1\nname = \"\"\n{}{}{}",
        "[[file]]\nid = \"a\"\nsource = \"a.bin\"\ndir = \"/opt\"\n",
        "[[file]]\nid = \"b\"\nsource = \"b.bin\"\ndir = \"/opt\"\nmode = \"9\"\n",
        "[[file]]\nid = \"c\"\nsource = \"c.bin\"\ndir = \"/opt\"\n",
    );
    Fx::new(&text, &[("a", b"a"), ("c", b"c")])
}

#[test]
fn plan_builds_passed_entries_and_carries_failures() {
    let fx = failing_fixture();
    let plan = fx.plan();
    let files: Vec<&str> = plan
        .entries()
        .iter()
        .filter_map(|e| match e {
            PlannedEntry::File { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(files, ["a", "c"]);
    assert_eq!(plan.left_out().len(), 1);
    assert_eq!(plan.left_out()[0].id.as_deref(), Some("b"));
    assert_eq!(plan.manifest_errors().len(), 1);
    assert!(plan.manifest_errors()[0].message.contains("name"));
    assert_eq!(plan.error_count(), 2);
}

#[test]
fn plan_refuses_when_no_entry_passed() {
    let cases = [
        "version = 2\nname = \"t\"\n[[file]]\nid = \"a\"\nsource = \"a.bin\"\ndir = \"/x\"\n",
        "version = 1\nname = \"t\"\n[[file]]\nid = \"a\"\nsource = \"a.bin\"\ndir = \"x\"\n",
        "version = 1\nname = \"t\"\n",
    ];
    for text in cases {
        let fx = Fx::new(text, &[("a", b"a")]);
        assert_eq!(
            ArchivePlan::new(&fx.report, &fx.assign).unwrap_err(),
            PlanError::NoEntries,
            "{text}"
        );
    }
}

#[test]
fn unassigned_ignores_failed_entries() {
    let fx = failing_fixture();
    assert!(fx.assign.get("b").is_none());
    assert!(ArchivePlan::new(&fx.report, &fx.assign).is_ok());
}

#[test]
fn all_unassigned_entries_refuse_the_plan() {
    let mut fx = simple();
    fx.assign.remove("a");
    fx.assign.remove("b");
    assert_eq!(
        ArchivePlan::new(&fx.report, &fx.assign).unwrap_err(),
        PlanError::Unassigned(vec!["a".into(), "b".into()])
    );
}

#[test]
fn partial_build_writes_only_assigned_entries_and_reports_the_rest() {
    let mut fx = simple();
    fx.assign.remove("b");
    let plan = ArchivePlan::new(&fx.report, &fx.assign).unwrap();
    assert_eq!(plan.not_loaded(), ["b"]);
    let (out, s) = fx.build(ArchiveFormat::Tar);
    let recs = read_back(&out, ArchiveFormat::Tar);
    assert!(
        !names(&recs).iter().any(|n| n.ends_with("b.bin")),
        "{:?}",
        names(&recs)
    );
    assert_eq!(s.built_ids, ["a"]);
    assert_eq!(s.not_loaded, ["b"]);
    assert_eq!(s.error_count, 0);
}

#[test]
fn build_with_errors_writes_only_passed_entries_and_reports_them() {
    let fx = failing_fixture();
    let (out, s) = fx.build(ArchiveFormat::TarGz);
    let recs = read_back(&out, ArchiveFormat::TarGz);
    assert_eq!(names(&recs), ["/opt/", "/opt/a.bin", "/opt/c.bin"]);
    assert_eq!(s.built_ids, ["a", "c"]);
    assert_eq!(s.left_out, fx.report.failures);
    assert_eq!(s.manifest_errors, fx.report.errors);
    assert_eq!(s.warnings, fx.report.warnings);
    assert_eq!(s.error_count, fx.report.error_count());
    assert_eq!(dir_listing(&fx.out_dir()), ["a.tar.gz"]);
}

#[test]
fn valid_build_reports_no_errors() {
    let fx = simple();
    let (_, s) = fx.build(ArchiveFormat::Tar);
    assert!(s.left_out.is_empty() && s.manifest_errors.is_empty() && s.not_loaded.is_empty());
    assert_eq!(s.error_count, 0);
    assert_eq!(s.built_ids, ["a", "b"]);
}

// ---- archive contents ----

#[test]
fn round_trip_preserves_headers() {
    for format in ALL {
        let fx = simple();
        let (out, s) = fx.build(format);
        let recs = read_back(&out, format);
        assert_eq!(
            names(&recs),
            [
                "/opt/",
                "/opt/gw/",
                "/opt/gw/bin/",
                "/opt/gw/bin/a",
                "/opt/gw/etc/",
                "/opt/gw/etc/b"
            ]
        );
        assert_eq!((s.entries, s.files, s.dirs), (6, 2, 4));
        let mtime = |id: &str| {
            let m = fs::metadata(fx.assign.get(id).unwrap())
                .unwrap()
                .modified()
                .unwrap();
            m.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
        };
        let newest = mtime("a").max(mtime("b"));
        for r in &recs[..3] {
            assert!(r.dir);
            assert_eq!((r.mode, r.uid, r.gid, r.mtime), (0o750, 7, 8, newest));
            assert_eq!((r.uname.as_str(), r.gname.as_str()), ("svc", "grp"));
        }
        let a = &recs[3];
        assert_eq!((a.mode, a.uid, a.gid, a.mtime), (0o755, 7, 8, mtime("a")));
        assert_eq!(a.data, b"alpha bytes\r\n");
        let b = &recs[5];
        assert_eq!(
            (b.mode, b.uid, b.uname.as_str(), b.gname.as_str()),
            (0o644, 1000, "bob", "grp")
        );
        assert_eq!(b.data, b"beta");
    }
}

#[test]
fn stored_names_are_absolute() {
    let fx = simple();
    for format in ALL {
        let (out, _) = fx.build_with(format, true, &Hooks::default());
        for r in read_back(&out, format) {
            assert_eq!(r.name[0], b'/');
        }
    }
}

fn long_fixture() -> Fx {
    let d99 = format!("/{}", "d".repeat(60)); // dir of 61 bytes
                                              // "/" + 60 d + "/" + name: 62 + name.len()
    let n100 = "n".repeat(100 - 62);
    let n150 = "m".repeat(150 - 62);
    let text = format!(
        "version = 1\nname = \"t\"\n[[file]]\nid = \"x\"\nsource = \"x.bin\"\ndir = \"{d99}\"\nname = \"{n100}\"\n[[file]]\nid = \"y\"\nsource = \"y.bin\"\ndir = \"{d99}\"\nname = \"{n150}\"\n"
    );
    Fx::valid(&text, &[("x", b"xx"), ("y", b"yy")])
}

#[test]
fn long_absolute_names_use_gnu_longlink() {
    let fx = long_fixture();
    let (out, _) = fx.build(ArchiveFormat::Tar);
    let raw = decompressed(&out, ArchiveFormat::Tar);
    let mut longlinks = 0;
    for block in raw.chunks(512) {
        if block.starts_with(b"././@LongLink\0") {
            assert_eq!(block[156], b'L');
            longlinks += 1;
        }
    }
    assert_eq!(longlinks, 2);
    let recs = read_back(&out, ArchiveFormat::Tar);
    let lens: Vec<usize> = recs.iter().map(|r| r.name.len()).collect();
    assert!(lens.contains(&100) && lens.contains(&150));
    assert!(recs.iter().all(|r| r.name[0] == b'/'));
}

#[test]
fn root_dir_never_emitted() {
    let fx = Fx::valid(&manifest(HDR, &[("a", "/", "")]), &[("a", b"a")]);
    let (out, _) = fx.build(ArchiveFormat::Tar);
    assert_eq!(names(&read_back(&out, ArchiveFormat::Tar)), ["/a"]);
}

#[test]
fn parent_dirs_emitted_once_in_order() {
    let fx = Fx::valid(&manifest(HDR, &[("a", "/a/b/c", "")]), &[("a", b"a")]);
    let (out, _) = fx.build(ArchiveFormat::Tar);
    assert_eq!(
        names(&read_back(&out, ArchiveFormat::Tar)),
        ["/a/", "/a/b/", "/a/b/c/", "/a/b/c/a"]
    );
}

#[test]
fn shared_parent_dirs_not_duplicated() {
    let fx = Fx::valid(
        &manifest(
            HDR,
            &[("a", "/a/b", ""), ("b", "/a/c", ""), ("c", "/a/b/", "")],
        ),
        &[("a", b"1"), ("b", b"2"), ("c", b"3")],
    );
    let (out, _) = fx.build(ArchiveFormat::Tar);
    assert_eq!(
        names(&read_back(&out, ArchiveFormat::Tar)),
        ["/a/", "/a/b/", "/a/b/a", "/a/c/", "/a/c/b", "/a/b/c"]
    );
}

#[test]
fn modes_come_from_manifest_not_source() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let fx = Fx::valid(
            &manifest(HDR, &[("a", "/x", "mode = \"0640\"")]),
            &[("a", b"a")],
        );
        fs::set_permissions(
            fx.assign.get("a").unwrap(),
            fs::Permissions::from_mode(0o777),
        )
        .unwrap();
        let (out, _) = fx.build(ArchiveFormat::Tar);
        assert_eq!(read_back(&out, ArchiveFormat::Tar)[1].mode, 0o640);
    }
    #[cfg(not(unix))]
    {
        let fx = Fx::valid(
            &manifest(HDR, &[("a", "/x", "mode = \"0640\"")]),
            &[("a", b"a")],
        );
        let (out, _) = fx.build(ArchiveFormat::Tar);
        assert_eq!(read_back(&out, ArchiveFormat::Tar)[1].mode, 0o640);
    }
}

// ---- failure and overwrite ----

#[test]
fn missing_source_writes_nothing() {
    let fx = simple();
    fs::remove_file(fx.assign.get("b").unwrap()).unwrap();
    let (_, r) = fx.build_with(ArchiveFormat::TarXz, false, &Hooks::default());
    match r {
        Err(BuildError::SourceMissing { id, .. }) => assert_eq!(id, "b"),
        other => panic!("{other:?}"),
    }
    assert!(dir_listing(&fx.out_dir()).is_empty());
}

#[test]
fn existing_output_requires_overwrite() {
    let fx = simple();
    let out = fx.out_dir().join("a.tar");
    fs::write(&out, b"precious").unwrap();
    let r = write_archive(&fx.plan(), &out, ArchiveFormat::Tar, false, |_| {});
    assert!(matches!(r, Err(BuildError::OutputExists { .. })));
    assert_eq!(fs::read(&out).unwrap(), b"precious");
    assert_eq!(dir_listing(&fx.out_dir()), ["a.tar"]);
}

#[test]
fn overwrite_replaces_atomically() {
    let fx = simple();
    let out = fx.out_dir().join("a.tar");
    fs::write(&out, b"old").unwrap();
    let s = write_archive(&fx.plan(), &out, ArchiveFormat::Tar, true, |_| {}).unwrap();
    assert_eq!(fs::metadata(&out).unwrap().len(), s.bytes);
    assert_eq!(s.sha256_hex, sha_hex(&out));
    assert_eq!(
        names(&read_back(&out, ArchiveFormat::Tar)),
        names(&read_back(
            &fx.build(ArchiveFormat::TarGz).0,
            ArchiveFormat::TarGz
        ))
    );
    assert_eq!(dir_listing(&fx.out_dir()), ["a.tar", "a.tar.gz"]);
}

#[test]
fn unreadable_source_writes_nothing() {
    let fx = simple();
    let b = fx.assign.get("b").unwrap().to_path_buf();
    fs::remove_file(&b).unwrap();
    fs::create_dir(&b).unwrap();
    let (_, r) = fx.build_with(ArchiveFormat::TarZst, false, &Hooks::default());
    match r {
        Err(BuildError::SourceUnreadable { id, .. }) => assert_eq!(id, "b"),
        other => panic!("{other:?}"),
    }
    assert!(dir_listing(&fx.out_dir()).is_empty());
}

#[test]
fn read_failure_while_writing_names_entry_and_writes_nothing() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("disk went away"))
        }
    }
    let fx = simple();
    let hooks = Hooks {
        open: Some(Box::new(|p, pass| {
            if pass == 0 {
                Ok(Box::new(fs::File::open(p)?) as Box<dyn Read>)
            } else {
                Ok(Box::new(Broken) as Box<dyn Read>)
            }
        })),
        ..Hooks::default()
    };
    let (_, r) = fx.build_with(ArchiveFormat::TarGz, false, &hooks);
    match r {
        Err(BuildError::SourceUnreadable { id, .. }) => assert_eq!(id, "a"),
        other => panic!("{other:?}"),
    }
    assert!(dir_listing(&fx.out_dir()).is_empty());
}

// Unit-level: a whole build with a 244-byte name exceeds Windows' MAX_PATH
// once the temp directory is prepended, so test the prefix itself.
#[test]
fn temp_prefix_caps_long_output_names() {
    let short = temp_prefix(Path::new("out/a.tar.gz"));
    assert_eq!(short, ".a.tar.gz.");
    let long = temp_prefix(&Path::new("out").join(format!("{}.tar", "o".repeat(240))));
    // tempfile adds 6 random bytes and the ".partial" suffix.
    assert!(long.len() + 6 + ".partial".len() <= 255);
    assert!(long.to_string_lossy().starts_with(".ooo"));
}

#[test]
fn temp_file_name_is_recognisable() {
    let fx = simple();
    let seen = std::cell::RefCell::new(Vec::new());
    let hooks = Hooks {
        fail_write_after: Some(2000),
        ..Hooks::default()
    };
    let out = fx.out_dir().join("a.tar");
    let _ = write_archive_with(
        &fx.plan(),
        &out,
        ArchiveFormat::Tar,
        false,
        &mut |_| seen.borrow_mut().extend(dir_listing(&fx.out_dir())),
        &hooks,
    );
    let seen = seen.into_inner();
    assert!(!seen.is_empty());
    assert!(seen
        .iter()
        .all(|n| n.starts_with(".a.tar.") && n.ends_with(".partial")));
    assert!(dir_listing(&fx.out_dir()).is_empty());
}

#[test]
fn interrupted_reads_are_retried() {
    struct Flaky<R>(R, bool);
    impl<R: std::io::Read> std::io::Read for Flaky<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.1 = !self.1;
            if self.1 {
                return Err(std::io::Error::from(std::io::ErrorKind::Interrupted));
            }
            self.0.read(buf)
        }
    }
    let fx = simple();
    let hooks = Hooks {
        open: Some(Box::new(|p, _| {
            Ok(Box::new(Flaky(fs::File::open(p)?, false)) as Box<dyn std::io::Read>)
        })),
        ..Hooks::default()
    };
    let (_, r) = fx.build_with(ArchiveFormat::Tar, false, &hooks);
    r.unwrap();
}

#[test]
fn injected_write_failure_leaves_nothing_and_names_entry() {
    let fx = simple();
    let hooks = Hooks {
        fail_write_after: Some(2000),
        ..Hooks::default()
    };
    let (_, r) = fx.build_with(ArchiveFormat::Tar, false, &hooks);
    match r {
        Err(BuildError::Io { id, .. }) => assert!(id.is_some()),
        other => panic!("{other:?}"),
    }
    assert!(dir_listing(&fx.out_dir()).is_empty());
}

#[test]
fn verify_failure_leaves_existing_output_unchanged() {
    let fx = simple();
    let out = fx.out_dir().join("a.tar");
    fs::write(&out, b"known bytes").unwrap();
    let old = SystemTime::now() - Duration::from_secs(86_400);
    fs::File::options()
        .write(true)
        .open(&out)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let before = fs::metadata(&out).unwrap().modified().unwrap();

    let hooks = Hooks {
        corrupt_at: Some(0),
        ..Hooks::default()
    };
    let r = write_archive_with(
        &fx.plan(),
        &out,
        ArchiveFormat::Tar,
        true,
        &mut |_| {},
        &hooks,
    );
    assert!(matches!(r, Err(BuildError::VerifyFailed(_))), "{r:?}");
    assert_eq!(fs::read(&out).unwrap(), b"known bytes");
    assert_eq!(fs::metadata(&out).unwrap().modified().unwrap(), before);
    assert_eq!(dir_listing(&fx.out_dir()), ["a.tar"]);

    let fresh = fx.tmp.path().join("fresh");
    fs::create_dir(&fresh).unwrap();
    let out2 = fresh.join("b.tar");
    let r = write_archive_with(
        &fx.plan(),
        &out2,
        ArchiveFormat::Tar,
        false,
        &mut |_| {},
        &hooks,
    );
    assert!(matches!(r, Err(BuildError::VerifyFailed(_))));
    assert!(!out2.exists());
    assert!(dir_listing(&fresh).is_empty());
}

#[test]
fn truncated_stream_fails_verification() {
    for format in [
        ArchiveFormat::TarGz,
        ArchiveFormat::TarZst,
        ArchiveFormat::TarXz,
    ] {
        let fx = simple();
        let hooks = Hooks {
            truncate_tail: 4,
            ..Hooks::default()
        };
        let (out, r) = fx.build_with(format, false, &hooks);
        assert!(
            matches!(r, Err(BuildError::VerifyFailed(_))),
            "{format:?}: {r:?}"
        );
        assert!(!out.exists());
        assert!(dir_listing(&fx.out_dir()).is_empty());
    }
}

// ---- summary ----

fn sha_hex(p: &Path) -> String {
    Sha256::digest(fs::read(p).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn summary_hash_matches_file() {
    let fx = simple();
    let (out, s) = fx.build(ArchiveFormat::TarZst);
    assert_eq!(s.sha256_hex, sha_hex(&out));
}

#[test]
fn summary_hash_computed_during_verify_matches_file() {
    for format in ALL {
        let fx = simple();
        let (out, s) = fx.build(format);
        assert_eq!(s.sha256_hex, sha_hex(&out), "{format:?}");
    }
}

#[test]
fn summary_reports_compressed_and_uncompressed_sizes() {
    let big = vec![b'z'; 200_000];
    let fx = Fx::valid(&manifest(HDR, &[("a", "/x", "")]), &[("a", &big)]);
    for format in ALL {
        let (out, s) = fx.build_with(format, true, &Hooks::default());
        let s = s.unwrap();
        assert_eq!(s.bytes, fs::metadata(&out).unwrap().len());
        assert_eq!(
            s.uncompressed_bytes,
            decompressed(&out, format).len() as u64
        );
        if format != ArchiveFormat::Tar {
            assert!(s.bytes < s.uncompressed_bytes);
        } else {
            assert_eq!(s.bytes, s.uncompressed_bytes);
        }
    }
}

#[test]
fn extract_command_matches_format() {
    let fx = simple();
    for format in ALL {
        let (_, s) = fx.build(format);
        assert_eq!(
            s.extract_command,
            format.extract_command(&format!("a{}", ext(format)))
        );
    }
}

// ---- eol ----

#[test]
fn eol_normalized_only_when_opted_in() {
    let src: &[u8] = b"a\r\nb\rc\r\n\r\nend\r";
    let fx = Fx::valid(
        &manifest(HDR, &[("a", "/x", "normalize_eol = true"), ("b", "/x", "")]),
        &[("a", src), ("b", src)],
    );
    let (out, s) = fx.build(ArchiveFormat::Tar);
    let recs = read_back(&out, ArchiveFormat::Tar);
    assert_eq!(recs[1].data, b"a\nb\rc\n\nend\r");
    assert_eq!(recs[2].data, src);
    assert_eq!(
        s.normalized_entries,
        [NormalizedEntry {
            id: "a".into(),
            crlf_replaced: 3
        }]
    );

    // A converted-size header for the normalised entry.
    let raw = decompressed(&out, ArchiveFormat::Tar);
    let h = tar::Header::from_byte_slice(&raw[512..512 * 2]);
    assert_eq!(h.size().unwrap(), recs[1].data.len() as u64);
}

#[test]
fn eol_handles_boundaries_and_edge_cases() {
    fn conv(input: &[u8], cap: usize) -> (Vec<u8>, u64) {
        let mut r = CrlfToLf::with_capacity(input, cap);
        let mut v = Vec::new();
        r.read_to_end(&mut v).unwrap();
        (v, r.replaced)
    }
    for cap in [1, 2, 3, 5, 64] {
        assert_eq!(conv(b"ab\r\ncd", cap), (b"ab\ncd".to_vec(), 1), "cap {cap}");
        assert_eq!(conv(b"\r\r\n", cap), (b"\r\n".to_vec(), 1), "cap {cap}");
        assert_eq!(conv(b"x\r", cap), (b"x\r".to_vec(), 0), "cap {cap}");
        assert_eq!(conv(b"\r", cap), (b"\r".to_vec(), 0), "cap {cap}");
        assert_eq!(conv(b"", cap), (Vec::new(), 0), "cap {cap}");
    }
    assert_eq!(converted_len(&b"a\r\nb"[..]).unwrap(), (3, 1));
    // A CRLF split across the big-buffer boundary.
    let mut big = vec![b'x'; 64 * 1024 - 1];
    big.extend_from_slice(b"\r\nyz");
    let (v, n) = conv(&big, 64 * 1024);
    assert_eq!(n, 1);
    assert_eq!(v.len(), big.len() - 1);
}

#[test]
fn eol_source_change_between_passes_fails() {
    let fx = Fx::valid(
        &manifest(HDR, &[("a", "/x", "normalize_eol = true")]),
        &[("a", b"one\r\ntwo\r\n")],
    );
    let hooks = Hooks {
        open: Some(Box::new(|p, pass| {
            if pass == 1 {
                Ok(Box::new(Cursor::new(b"one\r\ntwo\r\nmore".to_vec())))
            } else {
                Ok(Box::new(fs::File::open(p)?))
            }
        })),
        ..Hooks::default()
    };
    let (_, r) = fx.build_with(ArchiveFormat::Tar, false, &hooks);
    match r {
        Err(BuildError::SourceChanged { id }) => assert_eq!(id, "a"),
        other => panic!("{other:?}"),
    }
    assert!(dir_listing(&fx.out_dir()).is_empty());
}

// ---- progress ----

#[test]
fn progress_events_follow_contract() {
    let fx = Fx::valid(
        &manifest(HDR, &[("a", "/p/q/r", ""), ("b", "/p/s", "")]),
        &[("a", &vec![1u8; 100_000]), ("b", b"tiny")],
    );
    for format in ALL {
        let mut events = Vec::new();
        let out = fx.out_dir().join(format!("p{}", ext(format)));
        write_archive(&fx.plan(), &out, format, true, |p| events.push(p)).unwrap();

        let total = events[0].bytes_total;
        assert!(events.iter().all(|e| e.bytes_total == total));
        let split = events
            .iter()
            .position(|e| e.phase == BuildPhase::Verifying)
            .unwrap();
        let (w, v) = events.split_at(split);
        assert!(w.iter().all(|e| e.phase == BuildPhase::Writing));
        assert!(v.iter().all(|e| e.phase == BuildPhase::Verifying));
        for phase in [w, v] {
            assert!(phase.windows(2).all(|p| p[0].bytes_done <= p[1].bytes_done));
            let finals: Vec<_> = phase
                .iter()
                .enumerate()
                .filter(|(_, e)| e.bytes_done == total && e.entry_id.is_none())
                .collect();
            assert_eq!(finals.len(), 1, "{format:?}");
            assert_eq!(
                finals[0].0,
                phase.len() - 1,
                "final event is last of its phase"
            );
        }
        // Directory records report no entry id, before the first file.
        let first_file = w.iter().position(|e| e.entry_id.is_some()).unwrap();
        assert!(first_file >= 3 && w[..first_file].iter().all(|e| e.entry_id.is_none()));
        assert!(w.iter().any(|e| e.entry_id.as_deref() == Some("a")));
        assert!(w.iter().any(|e| e.entry_id.as_deref() == Some("b")));
        assert!(v[0].bytes_done <= total);
        assert_eq!(events.last().unwrap().phase, BuildPhase::Verifying);
    }
}

#[test]
fn file_progress_starts_after_long_name_record() {
    use super::header::{preamble_size, record_size, END_BLOCKS};
    let fx = long_fixture();
    let out = fx.out_dir().join("l.tar");
    let mut events = Vec::new();
    write_archive(&fx.plan(), &out, ArchiveFormat::Tar, false, |p| {
        events.push(p)
    })
    .unwrap();
    let total = events[0].bytes_total;
    let y_len = 150;
    let y_pos = total - END_BLOCKS - record_size(y_len, 2);
    assert!(preamble_size(y_len) > 512, "y has a long-name record");
    let first_y = events
        .iter()
        .find(|e| e.entry_id.as_deref() == Some("y"))
        .unwrap();
    assert_eq!(first_y.bytes_done, y_pos + preamble_size(y_len));
}

// ---- encoders ----

#[test]
fn zstd_frame_window_is_bounded() {
    let fx = simple();
    let (out, _) = fx.build(ArchiveFormat::TarZst);
    let b = fs::read(out).unwrap();
    assert_eq!(&b[..4], &[0x28, 0xb5, 0x2f, 0xfd]);
    let fhd = b[4];
    assert_eq!(
        fhd >> 5 & 1,
        0,
        "single-segment frames carry no window descriptor"
    );
    let wd = b[5];
    let log = 10 + u32::from(wd >> 3);
    let base = 1u64 << log;
    let window = base + base / 8 * u64::from(wd & 7);
    assert!(window <= 1 << 23, "window {window}");
    assert_eq!(fhd >> 2 & 1, 1, "content checksum flag");
}

#[test]
fn generated_types_use_number() {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/desktop/src/lib/generated"
    );
    for f in ["BuildSummary", "Progress", "NormalizedEntry"] {
        let text = fs::read_to_string(format!("{dir}/{f}.ts")).unwrap();
        assert!(!text.contains("bigint"), "{f}");
    }
    let s = fs::read_to_string(format!("{dir}/BuildSummary.ts")).unwrap();
    assert!(s.contains("errorCount: number"));
    assert!(s.contains("import type { Diagnostic }") && s.contains("import type { EntryFailure }"));
    assert!(!s.contains("export type Diagnostic") && !s.contains("export type EntryFailure"));
}

// ---- system tar (unix only) ----

#[cfg(unix)]
mod system_tar {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    fn works(cmd: &str, arg: &str) -> bool {
        Command::new(cmd)
            .arg(arg)
            .output()
            .is_ok_and(|o| o.status.success())
    }

    #[test]
    fn system_tar_lists_absolute_names() {
        if !works("tar", "--version") {
            eprintln!("skipped: no system tar");
            return;
        }
        let fx = long_fixture();
        let (out, _) = fx.build(ArchiveFormat::Tar);
        // Read-only listing; never extract with -P.
        let o = Command::new("tar").arg("-tPf").arg(&out).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let listing = String::from_utf8(o.stdout).unwrap();
        let lines: Vec<&str> = listing.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|l| l.starts_with('/')), "{listing}");
        assert!(lines.iter().any(|l| l.len() == 150));
    }

    #[test]
    fn extracts_with_system_tar_preserving_modes() {
        if !works("tar", "--version") {
            eprintln!("skipped: no system tar");
            return;
        }
        for format in ALL {
            let helper = match format {
                ArchiveFormat::Tar => None,
                ArchiveFormat::TarGz => Some("gzip"),
                ArchiveFormat::TarZst => Some("zstd"),
                ArchiveFormat::TarXz => Some("xz"),
            };
            if helper.is_some_and(|h| !works(h, "--version")) {
                eprintln!("skipped {format:?}: {} unavailable", helper.unwrap());
                continue;
            }
            let fx = simple();
            let (out, _) = fx.build(format);
            let dest = TempDir::new().unwrap();
            // No -P: the leading `/` is stripped into `dest`.
            let o = Command::new("tar")
                .arg("-xpf")
                .arg(&out)
                .arg("-C")
                .arg(dest.path())
                .output()
                .unwrap();
            assert!(
                o.status.success(),
                "{format:?}: {}",
                String::from_utf8_lossy(&o.stderr)
            );
            let mode = |p: &str| {
                fs::metadata(dest.path().join(p))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o7777
            };
            assert_eq!(mode("opt/gw/bin/a"), 0o755);
            assert_eq!(mode("opt/gw/etc/b"), 0o644);
            assert_eq!(mode("opt/gw/bin"), 0o750);
            assert_eq!(fs::read(dest.path().join("opt/gw/etc/b")).unwrap(), b"beta");
        }
    }
}
