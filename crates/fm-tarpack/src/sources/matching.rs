//! Matching dropped files and folders to manifest entries by file name.
//!
//! Nothing here guesses: a name that fits several entries, or an entry that
//! several files fit, is reported as ambiguous and assigned to none.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::assignments::{Assignments, EntryStatus};
use crate::manifest::Manifest;

/// A dropped folder is searched this many levels deep. Its direct children are
/// level 1, so a file at level 8 is found and one at level 9 is not.
pub const MAX_DEPTH: usize = 8;

/// An entry that more than one file could fill, or a file that more than one
/// entry could take. Assigned to nothing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct Ambiguity {
    pub id: String,
    pub candidates: Vec<PathBuf>,
}

/// A dropped path that was not assigned, with the reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct Unmatched {
    pub path: PathBuf,
    pub reason: String,
}

/// What a drop would do. Computed without changing anything; see [`apply`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
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

/// Collects files under `dir`, never following symlinks or junctions.
fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    let mut children: Vec<_> = read.filter_map(Result::ok).collect();
    children.sort_by_key(|e| e.file_name());
    for child in children {
        let Ok(ft) = child.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_file() {
            out.push(child.path());
        } else if ft.is_dir() && depth < MAX_DEPTH {
            walk(&child.path(), depth + 1, out);
        }
    }
}

/// Works out which dropped files go to which entries. Applies nothing.
///
/// Only entries that are unassigned or `Missing` are candidates: a drop never
/// replaces a `Ready` assignment.
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

    // (file, was dropped directly). Folder contents are not reported as
    // unmatched: a build folder holds plenty of unrelated files.
    let mut files: Vec<(PathBuf, bool)> = Vec::new();
    for path in dropped {
        match fs::symlink_metadata(path) {
            Err(_) => out.unmatched.push(Unmatched {
                path: path.clone(),
                reason: "not found".into(),
            }),
            Ok(md) if md.file_type().is_symlink() => out.unmatched.push(Unmatched {
                path: path.clone(),
                reason: "link not followed".into(),
            }),
            Ok(md) if md.is_dir() => {
                let mut found = Vec::new();
                walk(path, 1, &mut found);
                let useful = found
                    .iter()
                    .any(|f| lower_name(f).is_some_and(|n| by_name.contains_key(&n)));
                if !useful {
                    out.unmatched.push(Unmatched {
                        path: path.clone(),
                        reason: "no file in the folder matches an entry".into(),
                    });
                }
                files.extend(found.into_iter().map(|f| (f, false)));
            }
            Ok(_) => files.push((path.clone(), true)),
        }
    }

    // entry id -> candidate files; file -> open entries it fits
    let mut candidates: BTreeMap<&str, Vec<PathBuf>> = BTreeMap::new();
    let mut file_fits: BTreeMap<PathBuf, BTreeSet<&str>> = BTreeMap::new();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    for (file, direct) in &files {
        if !seen.insert(file.clone()) {
            continue;
        }
        let entries = lower_name(file).and_then(|n| by_name.get(&n));
        let open: Vec<&str> = entries
            .into_iter()
            .flatten()
            .filter(|(_, open)| *open)
            .map(|(id, _)| *id)
            .collect();
        if open.is_empty() {
            if *direct {
                let reason = if entries.is_some() {
                    "already assigned"
                } else {
                    "no entry matches this file name"
                };
                out.unmatched.push(Unmatched {
                    path: file.clone(),
                    reason: reason.into(),
                });
            }
            continue;
        }
        for id in &open {
            candidates.entry(id).or_default().push(file.clone());
        }
        file_fits.entry(file.clone()).or_default().extend(open);
    }

    for (id, cands) in candidates {
        let shared = cands.iter().any(|f| file_fits[f].len() > 1);
        if cands.len() == 1 && !shared {
            out.matched.push((id.to_owned(), cands[0].clone()));
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
