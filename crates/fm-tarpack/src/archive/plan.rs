//! Turns a whole parse report into the ordered list of archive records.

use std::fmt;
use std::path::PathBuf;

use crate::manifest::{Diagnostic, EntryFailure, Owner, ParseReport};
use crate::sources::Assignments;

/// One record of the archive. `path` is the absolute stored name; directory
/// names end in `/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedEntry {
    Dir {
        path: String,
    },
    File {
        id: String,
        source: PathBuf,
        path: String,
        mode: u32,
        owner: Owner,
        normalize_eol: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// No entry passed validation: entries withheld, every entry failed, or
    /// the manifest lists none.
    NoEntries,
    /// Passed entries with no source assigned.
    Unassigned(Vec<String>),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::NoEntries => write!(
                f,
                "no entry passed validation, so there is nothing to build"
            ),
            PlanError::Unassigned(ids) => {
                write!(f, "no source file for: {}", ids.join(", "))
            }
        }
    }
}

impl std::error::Error for PlanError {}

/// The ordered records, plus the rest of the report, which becomes the build's
/// final report. There is no constructor from a bare `Manifest`, so what was
/// left out cannot be lost on the way to a build.
#[derive(Clone, Debug)]
pub struct ArchivePlan {
    entries: Vec<PlannedEntry>,
    dir_mode: u32,
    dir_owner: Owner,
    left_out: Vec<EntryFailure>,
    manifest_errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
    error_count: u32,
}

impl ArchivePlan {
    /// Plans the passed entries of `report`. Errors in the report do not
    /// refuse the plan; they are carried along.
    pub fn new(report: &ParseReport, assignments: &Assignments) -> Result<ArchivePlan, PlanError> {
        let manifest = &report.manifest;
        if manifest.entries().is_empty() {
            return Err(PlanError::NoEntries);
        }
        let unassigned: Vec<String> = manifest
            .entries()
            .iter()
            .filter(|e| assignments.get(e.id()).is_none())
            .map(|e| e.id().to_string())
            .collect();
        if !unassigned.is_empty() {
            return Err(PlanError::Unassigned(unassigned));
        }

        let mut entries = Vec::new();
        let mut emitted: Vec<String> = Vec::new();
        for e in manifest.entries() {
            let mut dir = String::from("/");
            for seg in e.target_dir().split('/').filter(|s| !s.is_empty()) {
                dir.push_str(seg);
                dir.push('/');
                if !emitted.contains(&dir) {
                    emitted.push(dir.clone());
                    entries.push(PlannedEntry::Dir { path: dir.clone() });
                }
            }
            entries.push(PlannedEntry::File {
                id: e.id().to_string(),
                source: assignments
                    .get(e.id())
                    .expect("checked above")
                    .to_path_buf(),
                path: e.target_path(),
                mode: e.mode(),
                owner: e.owner().clone(),
                normalize_eol: e.normalize_eol(),
            });
        }
        Ok(ArchivePlan {
            entries,
            dir_mode: manifest.dir_mode(),
            dir_owner: manifest.default_owner().clone(),
            left_out: report.failures.clone(),
            manifest_errors: report.errors.clone(),
            warnings: report.warnings.clone(),
            error_count: report.error_count(),
        })
    }

    pub fn entries(&self) -> &[PlannedEntry] {
        &self.entries
    }
    pub fn dir_mode(&self) -> u32 {
        self.dir_mode
    }
    pub fn dir_owner(&self) -> &Owner {
        &self.dir_owner
    }
    pub fn left_out(&self) -> &[EntryFailure] {
        &self.left_out
    }
    pub fn manifest_errors(&self) -> &[Diagnostic] {
        &self.manifest_errors
    }
    pub fn warnings(&self) -> &[Diagnostic] {
        &self.warnings
    }
    pub fn error_count(&self) -> u32 {
        self.error_count
    }
}
