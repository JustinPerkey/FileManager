//! Fails if `apps/desktop/src/lib/generated/` differs from a fresh ts-rs export,
//! and regenerates it on request (`UPDATE_GENERATED=1`).
//!
//! This crate owns the export because it depends on every `fm-*` crate, so it
//! sees every boundary type, including its own private ones.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

type Exporter = fn(&ts_rs::Config) -> Result<(), ts_rs::ExportError>;

/// One entry per root boundary type, written
/// `<path::Type as ts_rs::TS>::export_all`. `export_all` also writes the types
/// it references. Later tasks append to this list (fm-tarpack types from M3
/// onward, shell types from M6).
const EXPORTERS: &[Exporter] = &[
    <fm_tarpack::manifest::Diagnostic as ts_rs::TS>::export_all,
    <fm_tarpack::manifest::Severity as ts_rs::TS>::export_all,
    <fm_tarpack::format::ArchiveFormat as ts_rs::TS>::export_all,
    <fm_tarpack::manifest::ManifestView as ts_rs::TS>::export_all,
    <fm_tarpack::manifest::EntryFailure as ts_rs::TS>::export_all,
];

const COMMITTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/generated");

const REGENERATE: &str =
    "UPDATE_GENERATED=1 cargo test -p filemanager --lib generated_types_are_current";

fn export_fresh() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = ts_rs::Config::from_env().with_out_dir(tmp.path());
    for export in EXPORTERS {
        export(&cfg).expect("ts-rs export");
    }
    tmp
}

fn read_tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read_dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n != ".gitkeep") {
                let rel = path.strip_prefix(root).expect("prefix").to_path_buf();
                out.insert(rel, fs::read(&path).expect("read"));
            }
        }
    }
    out
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

/// Makes `target` hold exactly the `fresh` tree plus a top-level `.gitkeep`.
fn sync_dir(fresh: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(target)? {
        let entry = entry?;
        if entry.file_name() == ".gitkeep" {
            continue;
        }
        if entry.path().is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    copy_tree(fresh, target)?;
    let keep = target.join(".gitkeep");
    if !keep.exists() {
        fs::write(keep, b"")?;
    }
    Ok(())
}

#[test]
fn generated_types_are_current() {
    let committed = Path::new(COMMITTED);
    // Export first, so a failing export never half-clears the committed dir.
    let fresh = export_fresh();

    if std::env::var("UPDATE_GENERATED").is_ok_and(|v| v == "1") {
        assert!(
            std::env::var_os("CI").is_none(),
            "UPDATE_GENERATED must not be used in CI"
        );
        sync_dir(fresh.path(), committed).expect("sync generated dir");
        println!(
            "updated lib/generated: {} file(s) written",
            read_tree(fresh.path()).len()
        );
        return;
    }

    let have = read_tree(committed);
    let want = read_tree(fresh.path());
    if have == want {
        return;
    }
    let mut lines = Vec::new();
    for (path, bytes) in &want {
        match have.get(path) {
            None => lines.push(format!("  added:   {}", path.display())),
            Some(old) if old != bytes => lines.push(format!("  changed: {}", path.display())),
            Some(_) => {}
        }
    }
    for path in have.keys().filter(|p| !want.contains_key(*p)) {
        lines.push(format!("  removed: {}", path.display()));
    }
    panic!(
        "apps/desktop/src/lib/generated is stale:\n{}\nregenerate with:\n  {REGENERATE}",
        lines.join("\n")
    );
}

#[test]
fn sync_replaces_stale_files_and_keeps_gitkeep() {
    let target = tempfile::tempdir().expect("tempdir");
    let fresh = tempfile::tempdir().expect("tempdir");
    fs::write(target.path().join(".gitkeep"), b"").unwrap();
    fs::write(target.path().join("stale.ts"), b"old").unwrap();
    fs::create_dir_all(target.path().join("old_dir/inner")).unwrap();
    fs::write(target.path().join("old_dir/inner/x.ts"), b"old").unwrap();
    fs::write(fresh.path().join("New.ts"), b"new").unwrap();
    fs::create_dir_all(fresh.path().join("sub")).unwrap();
    fs::write(fresh.path().join("sub/Nested.ts"), b"nested").unwrap();

    sync_dir(fresh.path(), target.path()).expect("sync");

    assert!(target.path().join(".gitkeep").exists());
    assert_eq!(read_tree(target.path()), read_tree(fresh.path()));
    assert!(!target.path().join("old_dir").exists());
}
