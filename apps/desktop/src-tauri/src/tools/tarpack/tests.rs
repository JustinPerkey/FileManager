use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use fm_core::{AppDirs, Store};
use fm_tarpack::archive::{BuildPhase, BuildSummary, Progress};
use fm_tarpack::format::ArchiveFormat;
use fm_tarpack::sources::{Assignments, DropOutcome, RememberedState};
use fm_tarpack::{BuildError, PlanError};
use tempfile::TempDir;

use super::core::{Core, DropJob};
use super::progress::{coalescing, ProgressCoalescer, PROGRESS_INTERVAL};
use super::types::{
    ArchiveFormatOption, BuildBlockedReason as Blocked, TarpackError, TarpackErrorKind as K,
    TarpackSession,
};
use super::watch::{is_manifest_change, ManifestWatcher};

/// A temporary app-data dir plus a work dir for manifests, sources and outputs.
struct Env {
    tmp: TempDir,
}

impl Env {
    fn new() -> Env {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("work")).unwrap();
        Env { tmp }
    }

    fn core(&self) -> Core {
        Core::new(AppDirs::at(self.tmp.path().join("app")))
    }

    fn work(&self, name: &str) -> PathBuf {
        self.tmp.path().join("work").join(name)
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let p = self.work(name);
        fs::write(&p, text).unwrap();
        p
    }

    /// A source file with `content`, in a directory of its own.
    fn source(&self, name: &str, content: &[u8]) -> PathBuf {
        let dir = self.tmp.path().join("src");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        fs::write(&p, content).unwrap();
        p
    }

    /// An output directory that starts empty.
    fn out(&self, name: &str) -> PathBuf {
        let dir = self.tmp.path().join("out");
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }
}

const TWO_FILES: &str = "version = 1\nname = \"Two\"\n\
    [[file]]\nid = \"a\"\nsource = \"a.bin\"\ndir = \"/opt/x\"\n\
    [[file]]\nid = \"b\"\nsource = \"b.bin\"\ndir = \"/opt/y\"\n";

/// `a` is valid, `b` has a bad mode, and the name is empty.
const INVALID: &str = "version = 1\nname = \"\"\n\
    [[file]]\nid = \"a\"\nsource = \"a.bin\"\ndir = \"/opt/x\"\n\
    [[file]]\nid = \"b\"\nsource = \"b.bin\"\ndir = \"/opt/y\"\nmode = \"9\"\n";

fn manifest(s: &TarpackSession) -> &super::types::SessionManifest {
    s.manifest.as_ref().expect("manifest loaded")
}

fn ids(s: &TarpackSession) -> Vec<&str> {
    manifest(s)
        .entries
        .iter()
        .map(|e| e.view.id.as_str())
        .collect()
}

fn run_build(
    core: &mut Core,
    overwrite: bool,
    progress: impl FnMut(Progress),
) -> Result<BuildSummary, TarpackError> {
    let job = core.begin_build()?;
    let result = job.run(overwrite, progress);
    core.finish_build(&job, &result);
    result
}

fn build(core: &mut Core, overwrite: bool) -> Result<BuildSummary, TarpackError> {
    run_build(core, overwrite, |_| {})
}

/// Begins, runs, and finishes a drop with nothing in between.
fn drop_now(core: &mut Core, paths: &[PathBuf]) -> Result<DropOutcome, TarpackError> {
    let job = core.begin_drop()?;
    let outcome = job.run(paths);
    Ok(core.finish_drop(&job, outcome)?.expect("session unchanged"))
}

/// A core with `TWO_FILES` open, both entries assigned, and an output set.
fn ready(env: &Env, out: &str) -> Core {
    let m = env.write("m.toml", TWO_FILES);
    let mut core = env.core();
    core.open(&m).unwrap();
    core.assign("a", &env.source("a.bin", b"aaa")).unwrap();
    core.assign("b", &env.source("b.bin", b"bbb")).unwrap();
    core.set_output(&env.out(out)).unwrap();
    core
}

#[test]
fn session_restores_recent_manifest() {
    let env = Env::new();
    let m = env.write("m.toml", TWO_FILES);
    env.core().open(&m).unwrap();

    let mut restarted = env.core();
    let s = restarted.session();
    assert_eq!(manifest(&s).name, "Two");
    assert_eq!(manifest(&s).path, m.to_string_lossy());
    assert_eq!(
        restarted.recent_manifests(),
        vec![m.to_string_lossy().into_owned()]
    );
}

#[test]
fn open_restores_remembered_sources() {
    let env = Env::new();
    let m = env.write("m.toml", TWO_FILES);
    let a = env.source("a.bin", b"a");
    let mut core = env.core();
    core.open(&m).unwrap();
    core.assign("a", &a).unwrap();

    let mut restarted = env.core();
    restarted.open(&m).unwrap();
    let s = restarted.snapshot();
    let e = &manifest(&s).entries[0];
    assert_eq!(e.assigned.as_deref(), Some(a.to_string_lossy().as_ref()));
    assert_eq!(s.ready_count, 1);
    assert_eq!(s.total_count, 2);
}

#[test]
fn reload_keeps_assignments_by_id() {
    let env = Env::new();
    let m = env.write("m.toml", TWO_FILES);
    let mut core = env.core();
    core.open(&m).unwrap();
    core.assign("a", &env.source("a.bin", b"a")).unwrap();
    core.assign("b", &env.source("b.bin", b"b")).unwrap();

    env.write("m.toml", &TWO_FILES.replace("Two", "Renamed"));
    core.reload().unwrap();
    let s = core.snapshot();
    assert_eq!(manifest(&s).name, "Renamed");
    assert_eq!(s.ready_count, 2);

    // `b` leaves a valid manifest: its assignment goes with it.
    env.write(
        "m.toml",
        "version = 1\nname = \"One\"\n[[file]]\nid = \"a\"\nsource = \"a.bin\"\ndir = \"/opt/x\"\n",
    );
    core.reload().unwrap();
    let s = core.snapshot();
    assert_eq!(ids(&s), ["a"]);
    assert_eq!(s.ready_count, 1);
    env.write("m.toml", TWO_FILES);
    core.reload().unwrap();
    assert_eq!(core.snapshot().ready_count, 1, "b is not silently restored");
}

#[test]
fn can_build_reasons_in_order() {
    let env = Env::new();
    let mut core = env.core();
    let s = core.session();
    assert!(!s.can_build);
    assert_eq!(s.build_blocked_reason, Some(Blocked::NoManifest));

    let m = env.write("empty.toml", "version = 1\nname = \"E\"\n");
    core.open(&m).unwrap();
    assert_eq!(
        core.snapshot().build_blocked_reason,
        Some(Blocked::NoEntries)
    );

    let m = env.write("m.toml", TWO_FILES);
    core.open(&m).unwrap();
    // Nothing loaded yet: a partial archive needs at least one file.
    assert_eq!(
        core.snapshot().build_blocked_reason,
        Some(Blocked::NothingLoaded)
    );
    // One file of two is a valid partial archive.
    core.assign("a", &env.source("a.bin", b"a")).unwrap();
    assert_eq!(
        core.snapshot().build_blocked_reason,
        Some(Blocked::NoOutput)
    );
    let b = env.source("b.bin", b"b");
    core.assign("b", &b).unwrap();
    core.set_output(&env.out("o")).unwrap();
    let s = core.snapshot();
    assert!(s.can_build);
    assert_eq!(s.build_blocked_reason, None);

    // Leaving one entry unassigned is a partial build, not a blocker.
    core.clear("b").unwrap();
    assert!(core.snapshot().can_build);
    core.assign("b", &b).unwrap();

    // A source that vanished is missing, not ready.
    fs::remove_file(&b).unwrap();
    let s = core.snapshot();
    assert!(!s.can_build);
    assert_eq!(s.build_blocked_reason, Some(Blocked::EntriesNotReady));
}

#[test]
fn invalid_manifest_session_carries_passed_and_failed_entries() {
    let env = Env::new();
    let m = env.write("gateway.toml", INVALID);
    let mut core = env.core();
    core.open(&m).unwrap();
    let s = core.snapshot();
    let mf = manifest(&s);
    assert_eq!(ids(&s), ["a"]);
    assert!(!mf.entries_withheld);
    assert_eq!(mf.failed_entries.len(), 1);
    assert_eq!(mf.failed_entries[0].id.as_deref(), Some("b"));
    assert_eq!(mf.errors.len(), 1, "the name error");
    assert_eq!(mf.error_count, 2);
    assert_eq!(mf.name, "gateway");
    assert_eq!(s.total_count, 1);
    assert_eq!(s.build_blocked_reason, Some(Blocked::NothingLoaded));

    core.assign("a", &env.source("a.bin", b"a")).unwrap();
    core.set_output(&env.out("o.tar")).unwrap();
    let s = core.snapshot();
    assert!(s.can_build, "errors alone never block a build");
    assert_eq!(manifest(&s).error_count, 2);
}

#[test]
fn withheld_manifest_session_has_no_entries() {
    let env = Env::new();
    let syntax = "version = 1\nname = [\n";
    let v2 = TWO_FILES.replace("version = 1", "version = 2");
    for (name, text) in [("v2.toml", v2.as_str()), ("syntax.toml", syntax)] {
        let m = env.write(name, text);
        let mut core = env.core();
        core.open(&m).expect("a readable file opens");
        core.set_output(&env.out("o.tar")).unwrap();
        let s = core.snapshot();
        assert!(manifest(&s).entries_withheld, "{name}");
        assert!(manifest(&s).entries.is_empty(), "{name}");
        assert!(manifest(&s).error_count > 0, "{name}");
        assert!(!s.can_build);
        assert_eq!(s.build_blocked_reason, Some(Blocked::NoEntries), "{name}");
    }
    let m = env.write("none.toml", "version = 1\nname = \"N\"\n");
    let mut core = env.core();
    core.open(&m).unwrap();
    core.set_output(&env.out("o.tar")).unwrap();
    let s = core.snapshot();
    assert!(!manifest(&s).entries_withheld);
    assert_eq!(s.build_blocked_reason, Some(Blocked::NoEntries));
}

#[test]
fn unreadable_manifest_fails_to_open() {
    let env = Env::new();
    let mut core = env.core();
    let err = core.open(&env.work("missing.toml")).unwrap_err();
    assert_eq!(err.kind, K::ManifestUnreadable);
    assert!(core.snapshot().manifest.is_none());
}

#[test]
fn assign_rejects_failed_entry_id() {
    let env = Env::new();
    let m = env.write("m.toml", INVALID);
    let mut core = env.core();
    core.open(&m).unwrap();
    let f = env.source("b.bin", b"b");
    for err in [
        core.assign("b", &f).unwrap_err(),
        core.clear("b").unwrap_err(),
    ] {
        assert_eq!(err.kind, K::UnknownEntry);
        assert_eq!(err.entry_id.as_deref(), Some("b"));
    }
    assert_eq!(core.assign("nope", &f).unwrap_err().kind, K::UnknownEntry);
    let err = core.assign("a", &env.work("not-there")).unwrap_err();
    assert_eq!(err.kind, K::NotAFile);
    assert_eq!(err.entry_id.as_deref(), Some("a"));
    let err = core.assign("a", env.work("").as_path()).unwrap_err();
    assert_eq!(err.kind, K::NotAFile, "a directory is not a file");
}

#[test]
fn clear_removes_only_the_assignment() {
    let env = Env::new();
    let mut core = ready(&env, "o.tar");
    core.clear("a").unwrap();
    let s = core.snapshot();
    assert_eq!(manifest(&s).entries[0].assigned, None);
    assert!(manifest(&s).entries[1].assigned.is_some());
    assert!(s.output_path.is_some());
}

#[test]
fn assign_dropped_matches_by_name() {
    let env = Env::new();
    let m = env.write("m.toml", INVALID);
    let mut core = env.core();
    core.open(&m).unwrap();
    let a = env.source("a.bin", b"a");
    let b = env.source("b.bin", b"b");
    let outcome = drop_now(&mut core, &[a.clone(), b]).unwrap();
    assert_eq!(outcome.matched, vec![("a".to_owned(), a)]);
    assert_eq!(outcome.unmatched.len(), 1, "b is a failed entry: no match");
    assert_eq!(core.snapshot().ready_count, 1);
}

#[test]
fn build_with_errors_builds_passed_entries_and_reports() {
    let env = Env::new();
    let m = env.write("m.toml", INVALID);
    let mut core = env.core();
    core.open(&m).unwrap();
    core.assign("a", &env.source("a.bin", b"hello")).unwrap();
    let out = env.out("o.tar");
    core.set_output(&out).unwrap();
    let session = core.snapshot();

    let summary = build(&mut core, false).expect("errors do not block a build");
    assert_eq!(summary.built_ids, ["a"]);
    assert_eq!(summary.left_out, manifest(&session).failed_entries);
    assert_eq!(summary.manifest_errors, manifest(&session).errors);
    assert_eq!(summary.error_count, 2);
    assert_eq!(summary.error_count, manifest(&session).error_count);

    let mut names = Vec::new();
    let mut ar = tar::Archive::new(fs::File::open(&out).unwrap());
    for e in ar.entries().unwrap() {
        names.push(e.unwrap().path().unwrap().to_string_lossy().into_owned());
    }
    assert_eq!(names, ["/opt/", "/opt/x/", "/opt/x/a.bin"]);

    let files: Vec<_> = fs::read_dir(out.parent().unwrap()).unwrap().collect();
    assert_eq!(files.len(), 1, "only the output file was written");
}

#[test]
fn build_refuses_when_no_entries() {
    let env = Env::new();
    let all_failed = "version = 1\nname = \"F\"\n[[file]]\nid = \"a\"\nsource = \"a\"\ndir = \"/o\"\nmode = \"9\"\n";
    let v2 = "version = 2\n";
    for (name, text) in [("failed.toml", all_failed), ("v2.toml", v2)] {
        let m = env.write(name, text);
        let mut core = env.core();
        core.open(&m).unwrap();
        let out = env.out(&format!("{name}.tar"));
        fs::write(&out, b"existing").unwrap();
        core.set_output(&out).unwrap();
        let err = build(&mut core, true).unwrap_err();
        assert_eq!(err.kind, K::NoEntries, "{name}");
        assert_eq!(fs::read(&out).unwrap(), b"existing");
    }
}

#[test]
fn invalid_manifest_keeps_remembered_source_of_failed_entry() {
    let env = Env::new();
    let m = env.write("m.toml", TWO_FILES);
    let a = env.source("a.bin", b"a");
    let b = env.source("b.bin", b"b");
    let mut core = env.core();
    core.open(&m).unwrap();
    core.assign("a", &a).unwrap();
    core.assign("b", &b).unwrap();

    // `b` breaks. Open, then assign `a` again (a mutating command persists).
    let broken = TWO_FILES.replace("dir = \"/opt/y\"", "dir = \"/opt/y\"\nmode = \"9\"");
    env.write("m.toml", &broken);
    let mut core = env.core();
    core.open(&m).unwrap();
    assert_eq!(ids(&core.snapshot()), ["a"]);
    core.assign("a", &a).unwrap();
    core.reload().unwrap();
    core.assign("a", &a).unwrap();

    // Restart, fix `b`, reload: its source comes back.
    let mut core = env.core();
    core.open(&m).unwrap();
    env.write("m.toml", TWO_FILES);
    core.reload().unwrap();
    let s = core.snapshot();
    assert_eq!(ids(&s), ["a", "b"]);
    assert_eq!(
        manifest(&s).entries[1].assigned.as_deref(),
        Some(b.to_string_lossy().as_ref())
    );
    assert_eq!(s.ready_count, 2);
}

#[test]
fn session_manifest_serialises_failure_fields() {
    let env = Env::new();
    let m = env.write("m.toml", INVALID);
    let mut core = env.core();
    core.open(&m).unwrap();
    let v = serde_json::to_value(core.snapshot()).unwrap();
    let mf = &v["manifest"];
    assert_eq!(mf["entriesWithheld"], false);
    assert_eq!(mf["errorCount"], 2);
    let failed = &mf["failedEntries"][0];
    for key in ["index", "id", "source", "line", "errors"] {
        assert!(failed.get(key).is_some(), "failedEntries[0].{key}");
    }
    assert_eq!(failed["id"], "b");
    assert_eq!(v["formats"].as_array().unwrap().len(), 4);
    assert_eq!(v["buildBlockedReason"], "nothingLoaded");
}

#[test]
fn session_entry_serialises_flat() {
    let env = Env::new();
    let m = env.write(
        "m.toml",
        "version = 1\nname = \"M\"\n[[file]]\nid = \"a\"\nsource = \"a.bin\"\ndir = \"/opt/x\"\nmode = \"0755\"\n",
    );
    let mut core = env.core();
    core.open(&m).unwrap();
    let v = serde_json::to_value(core.snapshot()).unwrap();
    let e = &v["manifest"]["entries"][0];
    assert_eq!(e["mode"], "0755");
    assert_eq!(e["modeText"], "rwxr-xr-x");
    assert_eq!(e["owner"], "root:root");
    assert_eq!(e["uid"], 0);
    assert_eq!(e["gid"], 0);
    assert_eq!(e["targetPath"], "/opt/x/a.bin");
    assert!(e.get("assigned").is_some());
    assert_eq!(e["status"], "unassigned");
}

#[test]
fn build_summary_serialises_report_fields() {
    let env = Env::new();
    let mut core = ready(&env, "o.tar");
    let v = serde_json::to_value(build(&mut core, false).unwrap()).unwrap();
    for key in [
        "builtIds",
        "leftOut",
        "manifestErrors",
        "warnings",
        "errorCount",
    ] {
        assert!(v.get(key).is_some(), "{key}");
    }
}

#[test]
fn build_refuses_when_manifest_changed() {
    let env = Env::new();
    let mut core = ready(&env, "o.tar");
    env.write("m.toml", &format!("{TWO_FILES}\n# edited\n"));
    let err = build(&mut core, false).unwrap_err();
    assert_eq!(err.kind, K::ManifestChangedOnDisk);
    assert!(!env.out("o.tar").exists());
    // Reloading accepts the edit, and the build goes through.
    core.reload().unwrap();
    build(&mut core, false).unwrap();
}

#[test]
fn build_refuses_existing_output_without_overwrite() {
    let env = Env::new();
    let mut core = ready(&env, "o.tar");
    let out = env.out("o.tar");
    fs::write(&out, b"precious").unwrap();
    let err = build(&mut core, false).unwrap_err();
    assert_eq!(err.kind, K::OutputExists);
    assert_eq!(fs::read(&out).unwrap(), b"precious");
    build(&mut core, true).expect("overwrite: true replaces it");
    assert_ne!(fs::read(&out).unwrap(), b"precious");
}

#[test]
fn build_refuses_while_another_is_running() {
    let env = Env::new();
    let mut core = ready(&env, "o.tar");
    let job = core.begin_build().unwrap();
    assert_eq!(
        core.begin_build().err().map(|e| e.kind),
        Some(K::BuildInProgress)
    );
    let result = job.run(false, |_| {});
    core.finish_build(&job, &result);
    core.begin_build().expect("free again");
}

#[test]
fn build_reports_missing_source_with_entry_id() {
    let env = Env::new();
    let mut core = ready(&env, "o.tar");
    fs::remove_file(env.source("b.bin", b"")).unwrap();
    let err = build(&mut core, false).unwrap_err();
    assert_eq!(err.kind, K::SourceMissing);
    assert_eq!(err.entry_id.as_deref(), Some("b"));
    assert!(!env.out("o.tar").exists());
}

#[test]
fn create_from_example_refuses_existing_path() {
    let env = Env::new();
    let existing = env.write("mine.toml", "keep me");
    let mut core = env.core();
    let err = core.create_from_example(&existing).unwrap_err();
    assert_eq!(err.kind, K::PathExists);
    assert_eq!(fs::read_to_string(&existing).unwrap(), "keep me");

    let fresh = env.work("new.toml");
    core.create_from_example(&fresh).unwrap();
    let s = core.snapshot();
    assert_eq!(manifest(&s).path, fresh.to_string_lossy());
    assert_eq!(manifest(&s).error_count, 0, "the bundled example is valid");
    assert_eq!(s.format, ArchiveFormat::TarZst, "from its output_name");
}

#[test]
fn set_format_rewrites_output_extension() {
    let env = Env::new();
    let m = env.write("m.toml", TWO_FILES);
    let mut core = env.core();
    core.open(&m).unwrap();
    core.set_output(&env.out("x.tar")).unwrap();
    core.set_format(ArchiveFormat::TarZst).unwrap();
    let s = core.snapshot();
    assert_eq!(s.format, ArchiveFormat::TarZst);
    assert_eq!(
        s.output_path.unwrap(),
        env.out("x.tar.zst").to_string_lossy()
    );
    core.set_format(ArchiveFormat::TarXz).unwrap();
    assert_eq!(
        core.snapshot().output_path.unwrap(),
        env.out("x.tar.xz").to_string_lossy()
    );

    // A `.tgz` path and a path with no suffix.
    for (from, to) in [
        ("x.tgz", "x.tar.zst"),
        ("noext", "noext.tar.zst"),
        ("X.TAR", "X.tar.zst"),
    ] {
        assert_eq!(
            super::core::with_format(Path::new(from), ArchiveFormat::TarZst),
            Path::new(to)
        );
    }

    // A remembered `.tgz` output is normalised on open.
    let dirs = AppDirs::at(env.tmp.path().join("app"));
    let (mut store, _) = Store::<RememberedState>::load(&dirs, "tarpack", "state").unwrap();
    let remembered = env.tmp.path().join("out.tgz");
    store.update(|r| {
        r.remember(
            &m,
            &Assignments::new(),
            Some(&remembered),
            ArchiveFormat::TarGz,
        )
    });
    store.save().unwrap();
    let mut core = env.core();
    core.open(&m).unwrap();
    let expected = env.tmp.path().join("out.tar.gz").display().to_string();
    assert_eq!(
        core.snapshot().output_path.as_deref(),
        Some(expected.as_str())
    );
}

#[test]
fn set_output_appends_or_switches_format() {
    let env = Env::new();
    let m = env.write("m.toml", TWO_FILES);
    let mut core = env.core();
    core.open(&m).unwrap();
    assert_eq!(core.snapshot().format, ArchiveFormat::Tar);

    core.set_output(&env.out("plain")).unwrap();
    let s = core.snapshot();
    assert_eq!(s.format, ArchiveFormat::Tar);
    assert_eq!(
        s.output_path.unwrap(),
        env.out("plain.tar").to_string_lossy()
    );

    core.set_output(&env.out("kept.tar")).unwrap();
    assert_eq!(
        core.snapshot().output_path.unwrap(),
        env.out("kept.tar").to_string_lossy()
    );

    core.set_output(&env.out("typed.tar.xz")).unwrap();
    let s = core.snapshot();
    assert_eq!(s.format, ArchiveFormat::TarXz);
    assert_eq!(
        s.output_path.unwrap(),
        env.out("typed.tar.xz").to_string_lossy()
    );

    core.set_output(&env.out("short.tgz")).unwrap();
    let s = core.snapshot();
    assert_eq!(s.format, ArchiveFormat::TarGz);
    assert_eq!(
        s.output_path.unwrap(),
        env.out("short.tar.gz").to_string_lossy()
    );

    // The typed format is what a restart restores.
    let mut restarted = env.core();
    restarted.open(&m).unwrap();
    assert_eq!(restarted.snapshot().format, ArchiveFormat::TarGz);
}

#[test]
fn format_restored_per_manifest() {
    let env = Env::new();
    let m1 = env.write("one.toml", TWO_FILES);
    let m2 = env.write("two.toml", TWO_FILES);
    let mut core = env.core();
    core.open(&m1).unwrap();
    core.set_format(ArchiveFormat::TarZst).unwrap();
    core.set_output(&env.out("o")).unwrap();

    let mut restarted = env.core();
    restarted.open(&m1).unwrap();
    let s = restarted.snapshot();
    assert_eq!(s.format, ArchiveFormat::TarZst);
    assert!(s.output_path.unwrap().ends_with(".tar.zst"));
    restarted.open(&m2).unwrap();
    let s = restarted.snapshot();
    assert_eq!(s.format, ArchiveFormat::Tar);
    assert_eq!(s.output_path, None);
}

#[test]
fn suggested_output_name_follows_format() {
    let env = Env::new();
    let with_name = env.write(
        "m.toml",
        "version = 1\nname = \"N\"\noutput_name = \"gateway.tar.zst\"\n",
    );
    let mut core = env.core();
    core.open(&with_name).unwrap();
    let s = core.snapshot();
    assert_eq!(s.format, ArchiveFormat::TarZst);
    assert_eq!(s.suggested_output_name.as_deref(), Some("gateway.tar.zst"));
    core.set_format(ArchiveFormat::TarXz).unwrap();
    assert_eq!(
        core.snapshot().suggested_output_name.as_deref(),
        Some("gateway.tar.xz")
    );

    let without = env.write("stem.toml", TWO_FILES);
    core.open(&without).unwrap();
    assert_eq!(
        core.snapshot().suggested_output_name.as_deref(),
        Some("stem.tar")
    );
    core.set_format(ArchiveFormat::TarGz).unwrap();
    assert_eq!(
        core.snapshot().suggested_output_name.as_deref(),
        Some("stem.tar.gz")
    );

    // `output_name` in error falls back to the stem.
    let bad = env.write("bad.toml", "version = 1\nname = \"N\"\noutput_name = 5\n");
    core.open(&bad).unwrap();
    let s = core.snapshot();
    assert_eq!(manifest(&s).output_name, None);
    assert!(s.suggested_output_name.unwrap().starts_with("bad."));
}

#[test]
fn no_manifest_session_defaults_and_rejects_format_and_output() {
    let env = Env::new();
    let mut core = env.core();
    let before = core.session();
    assert_eq!(before.format, ArchiveFormat::Tar);
    assert_eq!(before.formats.len(), 4);
    assert_eq!(before.output_path, None);
    assert_eq!(before.suggested_output_name, None);
    assert!(before.manifest.is_none());
    assert_eq!(
        core.set_format(ArchiveFormat::TarZst).unwrap_err().kind,
        K::NoManifest
    );
    assert_eq!(
        core.set_output(&env.out("o.tar")).unwrap_err().kind,
        K::NoManifest
    );
    assert_eq!(
        core.begin_build().err().map(|e| e.kind),
        Some(K::NoManifest)
    );
    assert_eq!(core.snapshot(), before);
}

#[test]
fn formats_carry_filter_extension() {
    let formats = ArchiveFormatOption::all();
    let got: Vec<_> = formats
        .iter()
        .map(|f| (f.format, f.extension.as_str(), f.filter_extension.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            (ArchiveFormat::Tar, ".tar", "tar"),
            (ArchiveFormat::TarGz, ".tar.gz", "gz"),
            (ArchiveFormat::TarZst, ".tar.zst", "zst"),
            (ArchiveFormat::TarXz, ".tar.xz", "xz"),
        ]
    );
    let v = serde_json::to_value(&formats[2]).unwrap();
    assert_eq!(v["filterExtension"], "zst");
}

#[test]
fn error_kinds_map_from_build_errors() {
    use std::io::Error as IoError;
    let p = || PathBuf::from("/x");
    let cases: Vec<(TarpackError, K, Option<&str>)> = vec![
        (
            BuildError::OutputExists { path: p() }.into(),
            K::OutputExists,
            None,
        ),
        (
            BuildError::SourceMissing {
                id: "i".into(),
                path: p(),
            }
            .into(),
            K::SourceMissing,
            Some("i"),
        ),
        (
            BuildError::SourceUnreadable {
                id: "i".into(),
                path: p(),
                cause: IoError::other("x"),
            }
            .into(),
            K::SourceUnreadable,
            Some("i"),
        ),
        (
            BuildError::SourceChanged { id: "i".into() }.into(),
            K::SourceChanged,
            Some("i"),
        ),
        (
            BuildError::Io {
                id: Some("i".into()),
                cause: IoError::other("x"),
            }
            .into(),
            K::Io,
            Some("i"),
        ),
        (
            BuildError::Io {
                id: None,
                cause: IoError::other("x"),
            }
            .into(),
            K::Io,
            None,
        ),
        (
            BuildError::VerifyFailed("d".into()).into(),
            K::VerifyFailed,
            None,
        ),
        (PlanError::NoEntries.into(), K::NoEntries, None),
        (
            PlanError::Unassigned(vec!["first".into(), "second".into()]).into(),
            K::EntriesNotReady,
            Some("first"),
        ),
    ];
    for (err, kind, id) in cases {
        assert_eq!(err.kind, kind, "{err}");
        assert_eq!(err.entry_id.as_deref(), id, "{err}");
        assert!(!err.message.is_empty());
    }
}

#[test]
fn error_serialises_with_optional_entry_id() {
    let with = serde_json::to_value(TarpackError::for_entry(K::NotAFile, "m", "a")).unwrap();
    assert_eq!(
        with,
        serde_json::json!({ "kind": "NotAFile", "message": "m", "entryId": "a" })
    );
    let without = serde_json::to_value(TarpackError::new(K::NoManifest, "m")).unwrap();
    assert_eq!(
        without,
        serde_json::json!({ "kind": "NoManifest", "message": "m" })
    );
}

fn ev(phase: BuildPhase, id: Option<&str>, done: u64, total: u64) -> Progress {
    Progress {
        phase,
        entry_id: id.map(str::to_owned),
        bytes_done: done,
        bytes_total: total,
    }
}

/// The M4 contract, which the coalescer must keep for what the UI receives.
fn assert_contract(events: &[Progress], file_ids: &[&str]) {
    assert!(!events.is_empty());
    let total = events[0].bytes_total;
    assert!(events.iter().all(|e| e.bytes_total == total));
    let split = events
        .iter()
        .position(|e| e.phase == BuildPhase::Verifying)
        .expect("verifying events");
    let (writing, verifying) = events.split_at(split);
    assert!(writing.iter().all(|e| e.phase == BuildPhase::Writing));
    assert!(verifying.iter().all(|e| e.phase == BuildPhase::Verifying));
    for (name, phase) in [("writing", writing), ("verifying", verifying)] {
        assert!(
            phase.windows(2).all(|w| w[0].bytes_done <= w[1].bytes_done),
            "{name} monotonic"
        );
        let finals: Vec<_> = phase
            .iter()
            .enumerate()
            .filter(|(_, e)| e.bytes_done == e.bytes_total && e.entry_id.is_none())
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            finals,
            [phase.len() - 1],
            "{name}: one final event, last of the phase"
        );
        for id in file_ids {
            assert!(
                phase.iter().any(|e| e.entry_id.as_deref() == Some(id)),
                "{name} carries {id}"
            );
        }
    }
    // Each phase starts over, and its first event may already be past 0: a
    // record is counted before the library reports it. Both phases process the
    // same records in the same order, so verifying is never further along.
    assert!(verifying[0].bytes_done < total, "verifying restarts");
    assert!(
        verifying[0].bytes_done <= writing[0].bytes_done,
        "verifying starts no later than writing"
    );
    assert_eq!(events.last().unwrap().phase, BuildPhase::Verifying);
}

#[test]
fn build_progress_events_follow_contract() {
    let env = Env::new();
    let mut core = ready(&env, "o.tar");
    let mut emitted = Vec::new();
    {
        let mut sink = coalescing(|e| emitted.push(e));
        run_build(&mut core, false, &mut sink).unwrap();
    }
    assert_contract(&emitted, &["a", "b"]);
}

#[test]
fn progress_coalescer_forwards_boundaries_and_throttles() {
    use BuildPhase::{Verifying as V, Writing as W};
    let base = Instant::now();
    let at = |ms: u64| base + Duration::from_millis(ms);
    let total = 1000;
    let mut input: Vec<(u64, Progress)> =
        vec![(0, ev(W, None, 0, total)), (1, ev(W, Some("a"), 0, total))];
    for t in 2..=52 {
        input.push((t, ev(W, Some("a"), t * 10, total)));
    }
    input.push((53, ev(W, Some("b"), 530, total)));
    input.push((54, ev(W, Some("b"), 540, total)));
    input.push((55, ev(W, None, total, total)));
    input.push((56, ev(V, Some("a"), 0, total)));
    input.push((57, ev(V, Some("a"), 100, total)));
    input.push((58, ev(V, Some("b"), 500, total)));
    input.push((59, ev(V, Some("b"), 600, total)));
    input.push((60, ev(V, None, total, total)));

    let mut c = ProgressCoalescer::new(PROGRESS_INTERVAL);
    let mut forwarded = Vec::new();
    for (t, e) in &input {
        if let Some(out) = c.offer(e.clone(), at(*t)) {
            assert_eq!(&out, e, "forwarded unchanged");
            forwarded.push(*t);
        }
    }
    // Phase starts (0, 56), file starts (1, 53, 58), finals (55, 60), and the
    // first chunk 50 ms after the last forwarded event (1 + 50).
    assert_eq!(forwarded, [0, 1, 51, 53, 55, 56, 58, 60]);

    let out: Vec<Progress> = input
        .iter()
        .filter(|(t, _)| forwarded.contains(t))
        .map(|(_, e)| e.clone())
        .collect();
    assert_contract(&out, &["a", "b"]);
}

#[test]
fn progress_coalescer_forwards_final_even_inside_interval() {
    use BuildPhase::Writing as W;
    let base = Instant::now();
    let mut c = ProgressCoalescer::new(PROGRESS_INTERVAL);
    assert!(c.offer(ev(W, Some("a"), 0, 10), base).is_some());
    assert!(c
        .offer(ev(W, Some("a"), 5, 10), base + Duration::from_millis(1))
        .is_none());
    let last = ev(W, None, 10, 10);
    assert_eq!(
        c.offer(last.clone(), base + Duration::from_millis(2)),
        Some(last)
    );
}

#[test]
fn build_progress_is_coalesced() {
    let env = Env::new();
    let m = env.write("m.toml", TWO_FILES);
    let mut core = env.core();
    core.open(&m).unwrap();
    core.assign("a", &env.source("a.bin", &vec![7u8; 4 << 20]))
        .unwrap();
    core.assign("b", &env.source("b.bin", b"small")).unwrap();
    core.set_output(&env.out("big.tar")).unwrap();

    let mut raw = Vec::new();
    run_build(&mut core, false, |e| raw.push(e)).unwrap();
    assert!(raw.len() > 500, "M4 reports every chunk: {}", raw.len());

    let base = Instant::now();
    let mut c = ProgressCoalescer::new(PROGRESS_INTERVAL);
    let forwarded: Vec<Progress> = raw
        .iter()
        .enumerate()
        .filter_map(|(i, e)| c.offer(e.clone(), base + Duration::from_millis(i as u64)))
        .collect();
    let limit = 2 + 2 + 2 * 2 + raw.len().div_ceil(50);
    assert!(
        forwarded.len() <= limit,
        "{} forwarded, limit {limit}",
        forwarded.len()
    );
    assert!(forwarded.len() * 4 < raw.len());
    assert_contract(&forwarded, &["a", "b"]);
}

#[test]
fn reveal_output_uses_stored_path() {
    let env = Env::new();
    let mut core = ready(&env, "o.tar");
    assert_eq!(core.reveal_target().unwrap_err().kind, K::NoOutput);
    build(&mut core, false).unwrap();
    assert_eq!(core.reveal_target().unwrap(), env.out("o.tar"));

    // A failed build does not replace it.
    let err = build(&mut core, false).unwrap_err();
    assert_eq!(err.kind, K::OutputExists);
    assert_eq!(core.reveal_target().unwrap(), env.out("o.tar"));
}

#[cfg(unix)]
#[test]
fn reveal_output_is_exact_for_non_unicode_names() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let env = Env::new();
    let m = env.write("m.toml", TWO_FILES);
    let mut core = env.core();
    core.open(&m).unwrap();
    core.assign("a", &env.source("a.bin", b"a")).unwrap();
    core.assign("b", &env.source("b.bin", b"b")).unwrap();
    let out = env.out("").join(OsStr::from_bytes(b"out\xff.tar"));
    core.set_output(&out).unwrap();
    let summary = build(&mut core, false).unwrap();
    assert!(summary.path.contains('\u{FFFD}'));
    let revealed = core.reveal_target().unwrap();
    assert_ne!(PathBuf::from(&summary.path), revealed);
    assert_eq!(revealed, out);
    assert!(revealed.exists());
}

#[test]
fn build_uses_session_format() {
    let env = Env::new();
    let mut core = ready(&env, "o.tar");
    core.set_format(ArchiveFormat::TarZst).unwrap();
    let summary = build(&mut core, false).unwrap();
    let out = env.out("o.tar.zst");
    assert_eq!(&fs::read(&out).unwrap()[..4], &[0x28, 0xB5, 0x2F, 0xFD]);
    assert_eq!(summary.format, ArchiveFormat::TarZst);
    assert!(summary.extract_command.contains("--zstd"));
    assert!(summary.extract_command.contains("o.tar.zst"));
}

#[test]
fn watcher_emits_once_per_edit() {
    let env = Env::new();
    let m = env.write("m.toml", TWO_FILES);
    let (tx, rx) = mpsc::channel();
    let _watcher = ManifestWatcher::start(&m, move || {
        let _ = tx.send(());
    })
    .expect("watch");
    // Let the watch settle, then edit once.
    std::thread::sleep(Duration::from_millis(200));
    fs::write(&m, format!("{TWO_FILES}# edit\n")).unwrap();
    rx.recv_timeout(Duration::from_secs(5)).expect("one event");
    assert!(
        rx.recv_timeout(Duration::from_millis(900)).is_err(),
        "exactly one event"
    );

    // Another file in the same directory is not this manifest.
    fs::write(env.work("other.toml"), "x").unwrap();
    assert!(rx.recv_timeout(Duration::from_millis(900)).is_err());
}

#[test]
fn only_a_settled_change_to_the_manifest_counts() {
    use notify_debouncer_mini::{DebouncedEvent, DebouncedEventKind as Kind};
    use std::ffi::OsStr;

    let ev = |p: &str, k| DebouncedEvent::new(PathBuf::from(p), k);
    let name = OsStr::new("m.toml");
    assert!(is_manifest_change(&[ev("/w/m.toml", Kind::Any)], name));
    assert!(!is_manifest_change(
        &[ev("/w/m.toml", Kind::AnyContinuous)],
        name
    ));
    assert!(!is_manifest_change(&[ev("/w/other.toml", Kind::Any)], name));
    assert!(is_manifest_change(
        &[ev("/w/other.toml", Kind::Any), ev("/w/m.toml", Kind::Any)],
        name
    ));
}

#[test]
fn drop_job_is_send_and_static() {
    fn assert_send_static<T: Send + 'static>() {}
    assert_send_static::<DropJob>();
}

/// A manifest with `a.bin` and `b.bin`, and a folder holding only `a.bin`.
fn drop_fixture(env: &Env) -> (PathBuf, PathBuf) {
    let m = env.write("m.toml", TWO_FILES);
    let folder = env.tmp.path().join("dropped");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("a.bin"), b"a").unwrap();
    (m, folder)
}

#[test]
fn stale_drop_is_not_applied() {
    let env = Env::new();
    let (m, folder) = drop_fixture(&env);
    let mut core = env.core();
    core.open(&m).unwrap();
    let job = core.begin_drop().unwrap();
    let b = env.source("b.bin", b"b");
    core.assign("b", &b).unwrap();
    let outcome = job.run(std::slice::from_ref(&folder));
    assert!(core.finish_drop(&job, outcome).unwrap().is_none());

    let s = core.snapshot();
    assert_eq!(manifest(&s).entries[0].assigned, None);
    assert_eq!(
        manifest(&s).entries[1].assigned.as_deref(),
        Some(b.to_string_lossy().as_ref())
    );
    let mut restarted = env.core();
    let s = restarted.session();
    assert_eq!(
        manifest(&s).entries[0].assigned,
        None,
        "a is not remembered"
    );

    let mut core = env.core();
    core.open(&m).unwrap();
    drop_now(&mut core, &[folder]).unwrap();
    assert!(manifest(&core.snapshot()).entries[0].assigned.is_some());
}

#[test]
fn drop_after_reopen_is_not_applied() {
    let env = Env::new();
    let (m1, folder) = drop_fixture(&env);
    let m2 = env.write("m2.toml", TWO_FILES);
    let mut core = env.core();
    core.open(&m1).unwrap();
    let job = core.begin_drop().unwrap();
    core.open(&m2).unwrap();
    let outcome = job.run(&[folder]);
    assert!(core.finish_drop(&job, outcome).unwrap().is_none());
    assert_eq!(manifest(&core.snapshot()).entries[0].assigned, None);
}

#[test]
fn drop_applies_when_session_unchanged() {
    let env = Env::new();
    let (m, folder) = drop_fixture(&env);
    let mut core = env.core();
    core.open(&m).unwrap();
    let job = core.begin_drop().unwrap();
    let outcome = job.run(&[folder]);
    let expected = outcome.matched.clone();
    let applied = core.finish_drop(&job, outcome).unwrap().expect("applied");
    assert_eq!(applied.matched, expected);
    assert!(manifest(&core.snapshot()).entries[0].assigned.is_some());

    let mut restarted = env.core();
    let s = restarted.session();
    assert!(manifest(&s).entries[0].assigned.is_some(), "persisted");
}

#[test]
fn write_new_file_removes_partial_file_on_failure() {
    use std::io::Write;
    let env = Env::new();
    let p = env.work("partial.toml");
    let err = super::core::write_new_file(&p, |f| {
        f.write_all(b"partial")?;
        Err(io::Error::other("injected"))
    })
    .unwrap_err();
    assert_eq!(err.kind, K::Io);
    assert!(err.message.contains("injected"));
    assert!(!p.exists());
}

#[test]
fn write_new_file_never_removes_existing_file() {
    let env = Env::new();
    let p = env.write("mine.toml", "keep me");
    let err = super::core::write_new_file(&p, |_| panic!("must not be called")).unwrap_err();
    assert_eq!(err.kind, K::PathExists);
    assert_eq!(fs::read_to_string(&p).unwrap(), "keep me");
}

#[test]
fn session_reports_unrestorable_recent_manifest() {
    let env = Env::new();
    let m = env.write("gone.toml", TWO_FILES);
    env.core().open(&m).unwrap();
    fs::remove_file(&m).unwrap();

    let mut core = env.core();
    let s = core.session();
    assert!(s.manifest.is_none());
    assert!(s.state_warning.as_deref().unwrap().contains("gone.toml"));
    assert_eq!(core.recent_manifests(), [m.to_string_lossy().into_owned()]);
}

#[test]
fn warnings_accumulate() {
    let mut core = Core::unpersisted("first.".into());
    core.add_warning("second.".into());
    assert_eq!(
        core.snapshot().state_warning.as_deref(),
        Some("first. second.")
    );
    core.add_warning("second.".into());
    assert_eq!(
        core.snapshot().state_warning.as_deref(),
        Some("first. second.")
    );
}

#[test]
fn reload_persists_remembered_state() {
    let env = Env::new();
    let m = env.write("m.toml", TWO_FILES);
    let mut core = env.core();
    core.open(&m).unwrap();
    core.assign("a", &env.source("a.bin", b"a")).unwrap();
    core.assign("b", &env.source("b.bin", b"b")).unwrap();
    env.write(
        "m.toml",
        "version = 1\nname = \"One\"\n[[file]]\nid = \"a\"\nsource = \"a.bin\"\ndir = \"/opt/x\"\n",
    );
    core.reload().unwrap();

    let (store, _) = Store::<RememberedState>::load(
        &AppDirs::at(env.tmp.path().join("app")),
        "tarpack",
        "state",
    )
    .unwrap();
    let all = store.get().restore_all(&m);
    assert!(all.get("a").is_some());
    assert!(all.get("b").is_none());
}
