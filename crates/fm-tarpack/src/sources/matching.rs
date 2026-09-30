//! Matching dropped files and folders to manifest entries by file name.
//!
//! Nothing here guesses: a name that fits several entries, or an entry that
//! several files fit, is reported as ambiguous and assigned to none.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use super::assignments::{Assignments, EntryStatus};
use crate::manifest::Manifest;

/// A dropped folder is searched this many levels deep. Its direct children are
/// level 1, so a file at level 8 is found and one at level 9 is not.
pub const MAX_DEPTH: usize = 8;

fn lossy<S: Serializer>(path: &Path, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&path.to_string_lossy())
}

fn lossy_vec<S: Serializer>(paths: &[PathBuf], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(paths.iter().map(|p| p.to_string_lossy()))
}

/// An entry that more than one file could fill, or a file that more than one
/// entry could take. Assigned to nothing.
///
/// Outbound only: the candidates are for display, serialized lossily, and
/// must not be read back as paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct Ambiguity {
    pub id: String,
    #[serde(serialize_with = "lossy_vec")]
    pub candidates: Vec<PathBuf>,
}

/// Why a dropped path was not assigned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum UnmatchedReason {
    /// The name fits only entries that are Ready; a drop never replaces them.
    AlreadyAssigned,
    /// A directly dropped file whose name fits no entry.
    NoEntry,
    /// A dropped path that does not exist.
    NotFound,
    /// A dropped path that is a symlink or junction; links are not followed.
    LinkNotFollowed,
    /// A dropped folder in which no file's name fits any entry.
    FolderNoMatch,
    /// A dropped path, or a directory or child met in a folder walk, that
    /// could not be read.
    Unreadable,
    /// A file that would be assigned, but its path is not valid Unicode.
    NotUnicode,
}

/// A dropped path that was not assigned, with the reason.
///
/// Outbound only: the path is for display, serialized lossily, and must not
/// be read back as a path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct Unmatched {
    #[serde(serialize_with = "lossy")]
    pub path: PathBuf,
    pub reason: UnmatchedReason,
}

/// What a drop would do. Computed without changing anything; see [`apply`].
///
/// Outbound only. `matched` holds only paths that are valid Unicode; the
/// paths in `unmatched` and `ambiguous` are display-only and lossy.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct DropOutcome {
    /// Entry id and the file to assign to it.
    pub matched: Vec<(String, PathBuf)>,
    pub unmatched: Vec<Unmatched>,
    pub ambiguous: Vec<Ambiguity>,
}

fn lower_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_lowercase)
}

fn unreadable(out: &mut Vec<Unmatched>, path: PathBuf) {
    out.push(Unmatched {
        path,
        reason: UnmatchedReason::Unreadable,
    });
}

/// Collects files under `dir`, never following symlinks or junctions.
/// Unreadable directories and children are reported and skipped. Returns
/// whether `dir` itself could be read.
fn walk(dir: &Path, depth: usize, files: &mut Vec<PathBuf>, problems: &mut Vec<Unmatched>) -> bool {
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => {
            unreadable(problems, dir.to_path_buf());
            return false;
        }
    };
    let mut children = Vec::new();
    for item in read {
        match item {
            Ok(e) => children.push(e),
            Err(_) => unreadable(problems, dir.to_path_buf()),
        }
    }
    children.sort_by_key(|e| e.file_name());
    for child in children {
        let Ok(ft) = child.file_type() else {
            unreadable(problems, child.path());
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_file() {
            files.push(child.path());
        } else if ft.is_dir() && depth < MAX_DEPTH {
            walk(&child.path(), depth + 1, files, problems);
        }
    }
    true
}

/// Works out which dropped files go to which entries. Applies nothing.
///
/// Only entries that are unassigned or `Missing` are candidates: a drop never
/// replaces a `Ready` assignment. A file whose path is not valid Unicode is
/// never put in `matched`; it is reported as `NotUnicode`.
pub fn match_dropped(
    manifest: &Manifest,
    current: &Assignments,
    dropped: &[PathBuf],
) -> DropOutcome {
    let mut out = DropOutcome::default();

    // lowercase source name -> (entry id, open?)
    let mut by_name: BTreeMap<String, Vec<(&str, bool)>> = BTreeMap::new();
    for e in manifest.entries() {
        let open = current.status(e.id()) != EntryStatus::Ready;
        by_name
            .entry(e.source().to_lowercase())
            .or_default()
            .push((e.id(), open));
    }
    let fits_any = |f: &PathBuf| lower_name(f).is_some_and(|n| by_name.contains_key(&n));

    // One record per path: (file, was dropped directly in any occurrence).
    // Unrelated files found in a folder are not reported.
    let mut files: Vec<(PathBuf, bool)> = Vec::new();
    let mut index: BTreeMap<PathBuf, usize> = BTreeMap::new();
    let mut add = |files: &mut Vec<(PathBuf, bool)>, path: PathBuf, direct: bool| {
        if let Some(&i) = index.get(&path) {
            files[i].1 |= direct;
        } else {
            index.insert(path.clone(), files.len());
            files.push((path, direct));
        }
    };
    for path in dropped {
        let reason = |r| Unmatched {
            path: path.clone(),
            reason: r,
        };
        match fs::symlink_metadata(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                out.unmatched.push(reason(UnmatchedReason::NotFound))
            }
            Err(_) => out.unmatched.push(reason(UnmatchedReason::Unreadable)),
            Ok(md) if md.file_type().is_symlink() => {
                out.unmatched.push(reason(UnmatchedReason::LinkNotFollowed))
            }
            Ok(md) if md.is_dir() => {
                let mut found = Vec::new();
                let readable = walk(path, 1, &mut found, &mut out.unmatched);
                if readable && !found.iter().any(fits_any) {
                    out.unmatched.push(reason(UnmatchedReason::FolderNoMatch));
                }
                for f in found {
                    add(&mut files, f, false);
                }
            }
            Ok(_) => add(&mut files, path.clone(), true),
        }
    }

    // entry id -> candidate files; file -> open entries it fits
    let mut candidates: BTreeMap<&str, Vec<PathBuf>> = BTreeMap::new();
    let mut file_fits: BTreeMap<&PathBuf, BTreeSet<&str>> = BTreeMap::new();
    for (file, direct) in &files {
        let Some(name) = lower_name(file) else {
            // Only a direct drop is reported; a walked file is unrelated.
            if *direct {
                out.unmatched.push(Unmatched {
                    path: file.clone(),
                    reason: if file.file_name().is_some() {
                        UnmatchedReason::NotUnicode
                    } else {
                        UnmatchedReason::NoEntry
                    },
                });
            }
            continue;
        };
        let entries = by_name.get(&name);
        let open: Vec<&str> = entries
            .into_iter()
            .flatten()
            .filter(|(_, open)| *open)
            .map(|(id, _)| *id)
            .collect();
        if open.is_empty() {
            if entries.is_some() {
                out.unmatched.push(Unmatched {
                    path: file.clone(),
                    reason: UnmatchedReason::AlreadyAssigned,
                });
            } else if *direct {
                out.unmatched.push(Unmatched {
                    path: file.clone(),
                    reason: UnmatchedReason::NoEntry,
                });
            }
            continue;
        }
        for id in &open {
            candidates.entry(id).or_default().push(file.clone());
        }
        file_fits.entry(file).or_default().extend(open);
    }

    for (id, cands) in candidates {
        let shared = cands.iter().any(|f| file_fits[f].len() > 1);
        if cands.len() == 1 && !shared {
            let file = cands.into_iter().next().expect("one candidate");
            if file.to_str().is_some() {
                out.matched.push((id.to_owned(), file));
            } else {
                out.unmatched.push(Unmatched {
                    path: file,
                    reason: UnmatchedReason::NotUnicode,
                });
            }
        } else {
            out.ambiguous.push(Ambiguity {
                id: id.to_owned(),
                candidates: cands,
            });
        }
    }
    out
}

/// Applies the `matched` part of an outcome. Ambiguous and unmatched files are
/// left for the user to decide.
pub fn apply(assignments: &mut Assignments, outcome: &DropOutcome) {
    for (id, path) in &outcome.matched {
        assignments.insert(id.clone(), path.clone());
    }
}
