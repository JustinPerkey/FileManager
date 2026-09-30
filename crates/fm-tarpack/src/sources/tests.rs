use std::fs;
use std::path::{Path, PathBuf};

use fm_core::{AppDirs, Store};
use tempfile::TempDir;

use super::*;
use crate::format::ArchiveFormat;
use crate::manifest::{parse, Manifest};

fn manifest_with(header: &str, entries: &[(&str, &str)]) -> Manifest {
    let mut t = format!("version = 1\nname = \"t\"\n{header}\n");
    for (id, src) in entries {
        t.push_str(&format!(
            "[[file]]\nid = \"{id}\"\nsource = \"{src}\"\nname = \"{id}\"\ndir = \"/opt\"\n"
        ));
    }
    let r = parse(&t);
    assert!(r.is_valid(), "{:?}", r.errors);
    r.manifest
}

fn manifest(entries: &[(&str, &str)]) -> Manifest {
    manifest_with("", entries)
}

fn touch(root: &Path, rel: &str) -> PathBuf {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, b"x").unwrap();
    p
}

#[test]
fn drop_matches_case_insensitively() {
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "App.BIN")]);
    let f = touch(tmp.path(), "app.bin");
    let out = match_dropped(&m, &Assignments::new(), std::slice::from_ref(&f));
    assert_eq!(out.matched, vec![("a".to_string(), f)]);
    assert!(out.unmatched.is_empty() && out.ambiguous.is_empty());
}

#[test]
fn drop_does_not_replace_ready_assignment() {
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "a.bin"), ("b", "b.bin")]);
    let old = touch(tmp.path(), "old/a.bin");
    let new = touch(tmp.path(), "new/a.bin");
    let mut asg = Assignments::new();
    asg.insert("a", old);
    let out = match_dropped(&m, &asg, std::slice::from_ref(&new));
    assert!(out.matched.is_empty());
    assert_eq!(out.unmatched.len(), 1);
    assert_eq!(out.unmatched[0].path, new);
    assert_eq!(out.unmatched[0].reason, "already assigned");

    // A Missing assignment is open again.
    let mut gone = Assignments::new();
    gone.insert("a", tmp.path().join("deleted.bin"));
    let out = match_dropped(&m, &gone, std::slice::from_ref(&new));
    assert_eq!(out.matched, vec![("a".to_string(), new)]);
}

#[test]
fn folder_drop_finds_nested_files() {
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "a.bin"), ("b", "b.bin")]);
    let a = touch(tmp.path(), "out/x/a.bin");
    let b = touch(tmp.path(), "out/b.bin");
    touch(tmp.path(), "out/other.txt");
    let out = match_dropped(&m, &Assignments::new(), &[tmp.path().join("out")]);
    assert_eq!(
        out.matched,
        vec![("a".to_string(), a), ("b".to_string(), b)]
    );
    assert!(out.unmatched.is_empty());
}

#[test]
fn folder_drop_respects_depth_cap() {
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "a.bin"), ("b", "b.bin")]);
    // Direct children of the dropped folder are level 1.
    let deep_ok = "d/".repeat(MAX_DEPTH - 1) + "a.bin";
    let too_deep = "d/".repeat(MAX_DEPTH) + "b.bin";
    let a = touch(&tmp.path().join("root"), &deep_ok);
    touch(&tmp.path().join("root"), &too_deep);
    let out = match_dropped(&m, &Assignments::new(), &[tmp.path().join("root")]);
    assert_eq!(out.matched, vec![("a".to_string(), a)]);
}

#[test]
fn ambiguous_drop_is_not_assigned() {
    let tmp = TempDir::new().unwrap();
    // Two files for one entry.
    let m = manifest(&[("a", "a.bin")]);
    let f1 = touch(tmp.path(), "one/a.bin");
    let f2 = touch(tmp.path(), "two/A.BIN");
    let out = match_dropped(&m, &Assignments::new(), &[f1.clone(), f2.clone()]);
    assert!(out.matched.is_empty());
    assert_eq!(out.ambiguous.len(), 1);
    assert_eq!(out.ambiguous[0].id, "a");
    assert_eq!(out.ambiguous[0].candidates, vec![f1, f2]);

    // One file that fits two entries.
    let m = manifest(&[("a", "a.bin"), ("b", "a.bin")]);
    let f = touch(tmp.path(), "three/a.bin");
    let out = match_dropped(&m, &Assignments::new(), &[f]);
    assert!(out.matched.is_empty());
    let ids: Vec<_> = out.ambiguous.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, ["a", "b"]);

    // Two files inside dropped folders.
    let m = manifest(&[("a", "a.bin")]);
    let out = match_dropped(
        &m,
        &Assignments::new(),
        &[tmp.path().join("one"), tmp.path().join("two")],
    );
    assert_eq!(out.ambiguous.len(), 1);
    assert!(out.matched.is_empty());
}

#[test]
fn unmatched_files_reported() {
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "a.bin")]);
    let f = touch(tmp.path(), "zzz.bin");
    let gone = tmp.path().join("nope");
    let empty = tmp.path().join("emptydir");
    fs::create_dir(&empty).unwrap();
    let out = match_dropped(
        &m,
        &Assignments::new(),
        &[f.clone(), gone.clone(), empty.clone()],
    );
    let paths: Vec<_> = out.unmatched.iter().map(|u| u.path.clone()).collect();
    assert_eq!(paths, vec![gone, empty, f]);
    assert!(out.matched.is_empty() && out.ambiguous.is_empty());
}

#[test]
fn pick_accepts_any_name() {
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "a.bin")]);
    let f = touch(tmp.path(), "whatever.dat");
    let mut asg = Assignments::new();
    assert_eq!(asg.status("a"), EntryStatus::Unassigned);
    asg.insert("a", f.clone());
    assert_eq!(asg.get("a"), Some(f.as_path()));
    assert_eq!(asg.status("a"), EntryStatus::Ready);
    // apply only applies matched.
    let out = match_dropped(&m, &Assignments::new(), &[touch(tmp.path(), "a.bin")]);
    let mut asg2 = Assignments::new();
    apply(&mut asg2, &out);
    assert_eq!(asg2.len(), 1);
}

fn store(root: &Path) -> Store<RememberedState> {
    Store::<RememberedState>::load(&AppDirs::at(root.to_path_buf()), "tarpack", "state")
        .unwrap()
        .0
}

#[test]
fn remembered_locations_restore_with_missing_status() {
    let tmp = TempDir::new().unwrap();
    let mpath = touch(tmp.path(), "m.toml");
    let m = manifest(&[("a", "a.bin"), ("b", "b.bin")]);
    let a = touch(tmp.path(), "a.bin");
    let b = touch(tmp.path(), "b.bin");
    let mut asg = Assignments::new();
    asg.insert("a", a);
    asg.insert("b", b.clone());
    let out = tmp.path().join("out.tar");

    let state_root = tmp.path().join("state");
    let mut s = store(&state_root);
    s.update(|st| st.remember(&mpath, &asg, Some(&out), ArchiveFormat::TarGz));
    s.save().unwrap();

    fs::remove_file(&b).unwrap();
    let s2 = store(&state_root);
    let restored = s2.get().restore(&mpath, &m);
    assert_eq!(restored.status("a"), EntryStatus::Ready);
    assert_eq!(restored.status("b"), EntryStatus::Missing);
    assert_eq!(s2.get().restore_output(&mpath), Some(out));
}

#[test]
fn removed_ids_are_pruned() {
    let tmp = TempDir::new().unwrap();
    let mpath = touch(tmp.path(), "m.toml");
    let full = manifest(&[("a", "a.bin"), ("b", "b.bin")]);
    let only_a = manifest(&[("a", "a.bin")]);
    let mut asg = Assignments::new();
    asg.insert("a", touch(tmp.path(), "a.bin"));
    asg.insert("b", touch(tmp.path(), "b.bin"));
    let mut st = RememberedState::default();
    st.remember(&mpath, &asg, None, ArchiveFormat::Tar);

    let restored = st.restore(&mpath, &only_a);
    assert!(restored.get("b").is_none());
    st.remember(&mpath, &restored, None, ArchiveFormat::Tar);
    assert!(st.restore(&mpath, &full).get("b").is_none());
}

#[test]
fn restore_all_keeps_ids_not_in_manifest() {
    let tmp = TempDir::new().unwrap();
    let mpath = touch(tmp.path(), "m.toml");
    let only_a = manifest(&[("a", "a.bin")]);
    let mut asg = Assignments::new();
    asg.insert("a", touch(tmp.path(), "a.bin"));
    asg.insert("b", touch(tmp.path(), "b.bin"));
    let root = tmp.path().join("state");
    let mut s = store(&root);
    s.update(|st| st.remember(&mpath, &asg, None, ArchiveFormat::Tar));
    s.save().unwrap();

    let s = store(&root);
    let all = s.get().restore_all(&mpath);
    assert!(all.get("a").is_some() && all.get("b").is_some());
    let filtered = s.get().restore(&mpath, &only_a);
    assert!(filtered.get("a").is_some() && filtered.get("b").is_none());

    let mut s = store(&root);
    s.update(|st| st.remember(&mpath, &all, None, ArchiveFormat::Tar));
    s.save().unwrap();
    let s = store(&root);
    assert!(s.get().restore_all(&mpath).get("b").is_some());
}

#[test]
fn recent_manifests_mru_capped() {
    let tmp = TempDir::new().unwrap();
    let mut st = RememberedState::default();
    let paths: Vec<_> = (0..12)
        .map(|i| touch(tmp.path(), &format!("m{i}.toml")))
        .collect();
    for p in &paths {
        st.touch_recent(p);
    }
    assert_eq!(st.recent_manifests.len(), MAX_RECENT);
    assert_eq!(st.recent_manifests[0], paths[11]);
    st.touch_recent(&paths[5]);
    assert_eq!(st.recent_manifests[0], paths[5]);
    assert_eq!(st.recent_manifests.len(), MAX_RECENT);
    assert_eq!(
        st.recent_manifests
            .iter()
            .filter(|p| **p == paths[5])
            .count(),
        1
    );
}

#[test]
fn last_format_restored_per_manifest() {
    let tmp = TempDir::new().unwrap();
    let m1 = touch(tmp.path(), "one.toml");
    let m2 = touch(tmp.path(), "two.toml");
    let man = manifest(&[("a", "a.bin")]);
    let root = tmp.path().join("state");
    let mut s = store(&root);
    s.update(|st| {
        st.remember(&m1, &Assignments::new(), None, ArchiveFormat::TarXz);
        st.remember(&m2, &Assignments::new(), None, ArchiveFormat::TarZst);
    });
    s.save().unwrap();
    let s = store(&root);
    assert_eq!(s.get().restore_format(&m1, &man), ArchiveFormat::TarXz);
    assert_eq!(s.get().restore_format(&m2, &man), ArchiveFormat::TarZst);
}

#[test]
fn format_falls_back_to_output_name_then_tar() {
    let tmp = TempDir::new().unwrap();
    let mp = touch(tmp.path(), "m.toml");
    let st = RememberedState::default();
    let plain = manifest(&[("a", "a.bin")]);
    assert_eq!(st.restore_format(&mp, &plain), ArchiveFormat::Tar);
    let named = manifest_with("output_name = \"pkg.tar.gz\"", &[("a", "a.bin")]);
    assert_eq!(st.restore_format(&mp, &named), ArchiveFormat::TarGz);
}

#[test]
fn state_without_last_format_loads() {
    let tmp = TempDir::new().unwrap();
    let mpath = touch(tmp.path(), "m.toml");
    let root = tmp.path().join("state");
    let key = mpath.canonicalize().unwrap().to_string_lossy().into_owned();
    let json = serde_json_escape(&key);
    let dir = root.join("tarpack");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("state.json"),
        format!(
            r#"{{"schema_version":1,"data":{{"per_manifest":{{"{json}":{{"sources":{{"a":"/x/a.bin"}}}}}},"recent_manifests":[]}}}}"#
        ),
    )
    .unwrap();
    let (s, warning) =
        Store::<RememberedState>::load(&AppDirs::at(root), "tarpack", "state").unwrap();
    assert!(warning.is_none());
    let man = manifest(&[("a", "a.bin")]);
    assert_eq!(
        s.get().restore(&mpath, &man).get("a"),
        Some(Path::new("/x/a.bin"))
    );
    assert_eq!(s.get().restore_format(&mpath, &man), ArchiveFormat::Tar);
    assert_eq!(s.get().restore_output(&mpath), None);
}

fn serde_json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(unix)]
#[test]
fn symlinks_not_followed() {
    use std::os::unix::fs::symlink;
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "a.bin"), ("b", "b.bin")]);
    let real = tmp.path().join("real");
    touch(&real, "a.bin");
    let root = tmp.path().join("root");
    fs::create_dir(&root).unwrap();
    symlink(&real, root.join("linkdir")).unwrap();
    symlink(real.join("a.bin"), root.join("b.bin")).unwrap();
    let out = match_dropped(&m, &Assignments::new(), std::slice::from_ref(&root));
    assert!(out.matched.is_empty() && out.ambiguous.is_empty());

    // A dropped symlink itself is not followed either.
    let out = match_dropped(&m, &Assignments::new(), &[root.join("b.bin")]);
    assert!(out.matched.is_empty());
    assert_eq!(out.unmatched.len(), 1);
}
