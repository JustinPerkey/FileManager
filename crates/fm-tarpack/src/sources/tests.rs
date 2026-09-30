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
    assert_eq!(out.unmatched[0].reason, UnmatchedReason::AlreadyAssigned);

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
    let got: Vec<_> = out
        .unmatched
        .iter()
        .map(|u| (u.path.clone(), u.reason))
        .collect();
    assert_eq!(
        got,
        vec![
            (gone, UnmatchedReason::NotFound),
            (empty, UnmatchedReason::FolderNoMatch),
            (f, UnmatchedReason::NoEntry),
        ]
    );
    assert!(out.matched.is_empty() && out.ambiguous.is_empty());
}

#[test]
fn pick_accepts_any_name() {
    let tmp = TempDir::new().unwrap();
    let f = touch(tmp.path(), "whatever.dat");
    let mut asg = Assignments::new();
    assert_eq!(asg.status("a"), EntryStatus::Unassigned);
    asg.insert("a", f.clone());
    assert_eq!(asg.get("a"), Some(f.as_path()));
    assert_eq!(asg.status("a"), EntryStatus::Ready);
    // apply only applies matched.
    let m = manifest(&[("a", "a.bin"), ("b", "x.bin"), ("c", "x.bin")]);
    let a = touch(tmp.path(), "d/a.bin");
    let x = touch(tmp.path(), "d/x.bin");
    let stray = touch(tmp.path(), "stray.txt");
    let out = match_dropped(
        &m,
        &Assignments::new(),
        &[a.clone(), x.clone(), stray.clone()],
    );
    assert_eq!(out.matched.len(), 1);
    assert_eq!(out.ambiguous.len(), 2);
    assert_eq!(out.unmatched.len(), 1);
    let mut asg2 = Assignments::new();
    apply(&mut asg2, &out);
    assert_eq!(asg2.len(), 1);
    assert_eq!(asg2.get("a"), Some(a.as_path()));
    assert_eq!(asg2.status("b"), EntryStatus::Unassigned);
    assert_eq!(asg2.status("c"), EntryStatus::Unassigned);
    assert!(asg2.iter().all(|(_, p)| p != stray && p != x));
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
    let only_a = manifest(&[("a", "a.bin")]);
    let mut asg = Assignments::new();
    asg.insert("a", touch(tmp.path(), "a.bin"));
    asg.insert("b", touch(tmp.path(), "b.bin"));
    let root = tmp.path().join("state");
    let mut s = store(&root);
    s.update(|st| st.remember(&mpath, &asg, None, ArchiveFormat::Tar));
    s.save().unwrap();

    let mut s = store(&root);
    let restored = s.get().restore(&mpath, &only_a);
    assert!(restored.get("b").is_none());
    s.update(|st| st.remember(&mpath, &restored, None, ArchiveFormat::Tar));
    s.save().unwrap();

    let s = store(&root);
    let all = s.get().restore_all(&mpath);
    assert!(all.get("a").is_some());
    assert!(all.get("b").is_none());
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
    let key = super::remembered::key(&mpath).unwrap();
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
    assert_eq!(out.unmatched[0].reason, UnmatchedReason::LinkNotFollowed);
}

#[test]
fn shared_candidate_is_ambiguous() {
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "x.bin"), ("b", "x.bin")]);
    let f = touch(tmp.path(), "dir/x.bin");
    let out = match_dropped(&m, &Assignments::new(), &[tmp.path().join("dir")]);
    assert!(out.matched.is_empty());
    assert_eq!(out.ambiguous.len(), 2);
    for (amb, id) in out.ambiguous.iter().zip(["a", "b"]) {
        assert_eq!(amb.id, id);
        assert_eq!(amb.candidates, vec![f.clone()]);
    }

    let mut asg = Assignments::new();
    asg.insert("a", touch(tmp.path(), "elsewhere/x.bin"));
    let out = match_dropped(&m, &asg, std::slice::from_ref(&f));
    assert_eq!(out.matched, vec![("b".to_string(), f)]);
    assert!(out.ambiguous.is_empty());
}

#[test]
fn folder_with_only_ready_matches_reports_already_assigned() {
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "a.bin"), ("b", "b.bin")]);
    let a = touch(tmp.path(), "dir/a.bin");
    let b = touch(tmp.path(), "dir/sub/b.bin");
    touch(tmp.path(), "dir/other.txt");
    let mut asg = Assignments::new();
    asg.insert("a", touch(tmp.path(), "old/a.bin"));
    asg.insert("b", touch(tmp.path(), "old/b.bin"));
    let out = match_dropped(&m, &asg, &[tmp.path().join("dir")]);
    assert!(out.matched.is_empty() && out.ambiguous.is_empty());
    let got: Vec<_> = out
        .unmatched
        .iter()
        .map(|u| (u.path.clone(), u.reason))
        .collect();
    assert_eq!(
        got,
        vec![
            (a, UnmatchedReason::AlreadyAssigned),
            (b, UnmatchedReason::AlreadyAssigned)
        ]
    );
}

#[test]
fn direct_drop_keeps_status_when_folder_comes_first() {
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "a.bin")]);
    let notes = touch(tmp.path(), "F/notes.txt");
    let f = tmp.path().join("F");
    let out = match_dropped(&m, &Assignments::new(), &[f.clone(), notes.clone()]);
    let no_entry: Vec<_> = out
        .unmatched
        .iter()
        .filter(|u| u.reason == UnmatchedReason::NoEntry)
        .map(|u| u.path.clone())
        .collect();
    assert_eq!(no_entry, vec![notes]);

    let a = touch(tmp.path(), "F/a.bin");
    let mut asg = Assignments::new();
    asg.insert("a", touch(tmp.path(), "old/a.bin"));
    let out = match_dropped(&m, &asg, &[f, a.clone()]);
    assert_eq!(
        out.unmatched,
        vec![Unmatched {
            path: a,
            reason: UnmatchedReason::AlreadyAssigned
        }]
    );
}

#[cfg(unix)]
#[test]
fn unreadable_directory_reported() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "a.bin")]);
    let a = touch(tmp.path(), "root/a.bin");
    let locked = tmp.path().join("root/locked");
    fs::create_dir(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let restore = || fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    if fs::read_dir(&locked).is_ok() {
        restore();
        return;
    }
    let out = match_dropped(&m, &Assignments::new(), &[tmp.path().join("root")]);
    restore();
    assert_eq!(out.matched, vec![("a".to_string(), a)]);
    assert_eq!(
        out.unmatched,
        vec![Unmatched {
            path: locked,
            reason: UnmatchedReason::Unreadable
        }]
    );
}

#[test]
fn unmatched_reason_serializes_camel_case() {
    use UnmatchedReason::*;
    let all = [
        (AlreadyAssigned, "alreadyAssigned"),
        (NoEntry, "noEntry"),
        (NotFound, "notFound"),
        (LinkNotFollowed, "linkNotFollowed"),
        (FolderNoMatch, "folderNoMatch"),
        (Unreadable, "unreadable"),
        (NotUnicode, "notUnicode"),
    ];
    for (reason, text) in all {
        assert_eq!(serde_json::to_value(reason).unwrap(), text);
    }
}

#[cfg(any(target_os = "linux", windows))]
fn bad_name() -> std::ffi::OsString {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::OsStr::from_bytes(b"\xff").to_os_string()
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&[0xD800])
    }
}

/// Creates a file at `dir/<bad>/a.bin`, or returns `None` if the filesystem
/// refuses such a name.
#[cfg(any(target_os = "linux", windows))]
fn bad_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let d = dir.join(bad_name());
    fs::create_dir_all(&d).ok()?;
    let f = d.join(name);
    fs::write(&f, b"x").ok()?;
    Some(f)
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn non_unicode_match_is_reported() {
    let tmp = TempDir::new().unwrap();
    let m = manifest(&[("a", "a.bin")]);
    let Some(f) = bad_file(&tmp.path().join("root"), "a.bin") else {
        return;
    };
    let out = match_dropped(&m, &Assignments::new(), &[tmp.path().join("root")]);
    assert!(out.matched.is_empty());
    assert_eq!(
        out.unmatched,
        vec![Unmatched {
            path: f,
            reason: UnmatchedReason::NotUnicode
        }]
    );
    assert!(serde_json::to_string(&out).is_ok());
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn outcome_serializes_display_paths_lossily() {
    let bad = PathBuf::from("dir").join(bad_name());
    let outcome = DropOutcome {
        matched: vec![],
        unmatched: vec![Unmatched {
            path: bad.clone(),
            reason: UnmatchedReason::NotUnicode,
        }],
        ambiguous: vec![Ambiguity {
            id: "a".into(),
            candidates: vec![bad],
        }],
    };
    let v = serde_json::to_value(&outcome).unwrap();
    let u = v["unmatched"][0]["path"].as_str().unwrap();
    let c = v["ambiguous"][0]["candidates"][0].as_str().unwrap();
    assert!(u.contains('\u{FFFD}') && c.contains('\u{FFFD}'));
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn non_unicode_paths_are_not_remembered() {
    let tmp = TempDir::new().unwrap();
    let mpath = touch(tmp.path(), "m.toml");
    let bad = tmp.path().join(bad_name());
    let root = tmp.path().join("state");
    let mut asg = Assignments::new();
    asg.insert("good", touch(tmp.path(), "good.bin"));
    asg.insert("bad", bad.clone());
    let mut s = store(&root);
    s.update(|st| {
        st.remember(&mpath, &asg, Some(&bad), ArchiveFormat::Tar);
        st.touch_recent(&bad);
        let before = st.per_manifest.clone();
        st.remember(&bad, &asg, None, ArchiveFormat::Tar);
        assert_eq!(st.per_manifest, before);
    });
    s.save().unwrap();

    let s = store(&root);
    let restored = s.get().restore_all(&mpath);
    assert!(restored.get("good").is_some());
    assert!(restored.get("bad").is_none());
    assert_eq!(s.get().restore_output(&mpath), None);
    assert!(!s.get().recent_manifests.contains(&bad));
    assert!(s.get().recent_manifests.is_empty());
}

#[cfg(windows)]
#[test]
fn key_ignores_case_and_verbatim_prefix() {
    let tmp = TempDir::new().unwrap();
    let plain = touch(tmp.path(), "m.toml");
    let upper = PathBuf::from(plain.to_str().unwrap().to_uppercase());
    let verbatim = plain.canonicalize().unwrap();
    let mut asg = Assignments::new();
    asg.insert("a", touch(tmp.path(), "a.bin"));
    let mut st = RememberedState::default();
    st.remember(&plain, &asg, None, ArchiveFormat::Tar);
    for p in [&upper, &verbatim] {
        assert_eq!(st.restore_all(p).get("a"), asg.get("a"));
    }
    for p in [&plain, &upper, &verbatim] {
        st.remember(p, &asg, None, ArchiveFormat::Tar);
        st.touch_recent(p);
    }
    assert_eq!(st.per_manifest.len(), 1);
    assert_eq!(st.recent_manifests.len(), 1);
}
