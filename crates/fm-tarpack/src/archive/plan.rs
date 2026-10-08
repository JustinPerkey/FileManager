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
    /// Passed entries exist but none has a source assigned, so a partial
    /// archive would be empty. Carries every passed entry's id.
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
                write!(
                    f,
                    "no source file chosen for any entry ({})",
                    ids.join(", ")
                )
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
    not_loaded: Vec<String>,
    manifest_errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
    error_count: u32,
}

impl ArchivePlan {
    /// Plans the passed entries of `report` that have a source assigned. The
    /// manifest lists every file that could be packaged, so entries with no
    /// source are a deliberate partial archive: they are left out and listed
    /// in `not_loaded`. Errors in the report do not refuse the plan; they are
    /// carried along.
    pub fn new(report: &ParseReport, assignments: &Assignments) -> Result<ArchivePlan, PlanError> {
        let manifest = &report.manifest;
        if manifest.entries().is_empty() {
            return Err(PlanError::NoEntries);
        }
        let (loaded, not_loaded): (Vec<_>, Vec<_>) = manifest
            .entries()
            .iter()
            .partition(|e| assignments.get(e.id()).is_some());
        if loaded.is_empty() {
            return Err(PlanError::Unassigned(
                not_loaded.iter().map(|e| e.id().to_string()).collect(),
            ));
        }
        let not_loaded: Vec<String> = not_loaded.iter().map(|e| e.id().to_string()).collect();

        let mut entries = Vec::new();
        let mut emitted: Vec<String> = Vec::new();
        for e in loaded {
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
            not_loaded,
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
    /// Ids of passed entries with no source, left out on purpose.
    pub fn not_loaded(&self) -> &[String] {
        &self.not_loaded
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
