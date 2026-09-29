//! The validated manifest. Fields are private and the constructors are
//! crate-private, so only validation can produce one.

use serde::{Deserialize, Serialize};

use super::mode;
use crate::format::ArchiveFormat;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Error,
    Warning,
}

/// A problem found in a manifest. `line` and `col` are 1-based.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub severity: Severity,
    pub line: u32,
    pub col: u32,
    pub entry_id: Option<String>,
    pub message: String,
}

/// One `[[file]]` table that failed validation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct EntryFailure {
    /// 1-based position of the `[[file]]` table.
    pub index: u32,
    /// The id, when it is a non-empty string.
    pub id: Option<String>,
    /// `source`, when it is a string, so a table without an id can still be
    /// recognised.
    pub source: Option<String>,
    /// Line of the table's `[[file]]` header (of the element, for an inline
    /// array).
    pub line: u32,
    /// Every error of the table, sorted by (line, col); never empty.
    pub errors: Vec<Diagnostic>,
}

/// Everything a parse found. Errors do not block a build: the archive is built
/// from `manifest`'s passed entries, and the failures and errors travel into
/// the build's result.
#[derive(Clone, Debug)]
pub struct ParseReport {
    /// Passed entries only, in manifest order.
    pub manifest: Manifest,
    /// A withholding error occurred: the manifest has no entries.
    pub entries_withheld: bool,
    /// Manifest-level errors, sorted by (line, col).
    pub errors: Vec<Diagnostic>,
    /// Failed `[[file]]` tables, in manifest order.
    pub failures: Vec<EntryFailure>,
    /// Every warning, sorted by (line, col).
    pub warnings: Vec<Diagnostic>,
}

impl ParseReport {
    /// No errors and no failures.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty() && self.failures.is_empty()
    }

    /// Every error counted once: manifest-level errors plus every failure's.
    pub fn error_count(&self) -> u32 {
        (self.errors.len() + self.failures.iter().map(|f| f.errors.len()).sum::<usize>()) as u32
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub uid: u32,
    pub gid: u32,
    pub uname: String,
    pub gname: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    id: String,
    source: String,
    target_dir: String,
    target_name: String,
    mode: u32,
    owner: Owner,
    normalize_eol: bool,
}

impl Entry {
    pub(crate) fn new(
        id: String,
        source: String,
        target_dir: String,
        target_name: String,
        mode: u32,
        owner: Owner,
        normalize_eol: bool,
    ) -> Entry {
        Entry {
            id,
            source,
            target_dir,
            target_name,
            mode,
            owner,
            normalize_eol,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    /// Expected Windows file name.
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Linux directory inside the archive, as written in the manifest.
    pub fn target_dir(&self) -> &str {
        &self.target_dir
    }
    pub fn target_name(&self) -> &str {
        &self.target_name
    }
    pub fn mode(&self) -> u32 {
        self.mode
    }
    pub fn owner(&self) -> &Owner {
        &self.owner
    }
    pub fn uid(&self) -> u32 {
        self.owner.uid
    }
    pub fn gid(&self) -> u32 {
        self.owner.gid
    }
    pub fn uname(&self) -> &str {
        &self.owner.uname
    }
    pub fn gname(&self) -> &str {
        &self.owner.gname
    }
    pub fn normalize_eol(&self) -> bool {
        self.normalize_eol
    }

    /// The absolute stored name: `dir` without its trailing `/`, then `/`,
    /// then the name.
    pub fn target_path(&self) -> String {
        format!(
            "{}/{}",
            self.target_dir.trim_end_matches('/'),
            self.target_name
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    name: String,
    output_name: Option<String>,
    dir_mode: u32,
    default_owner: Owner,
    entries: Vec<Entry>,
    complete: bool,
}

impl Manifest {
    pub(crate) fn new(
        name: String,
        output_name: Option<String>,
        dir_mode: u32,
        default_owner: Owner,
        entries: Vec<Entry>,
        complete: bool,
    ) -> Manifest {
        Manifest {
            name,
            output_name,
            dir_mode,
            default_owner,
            entries,
            complete,
        }
    }

    /// Whether the manifest holds every entry its file lists. A manifest that
    /// is not complete holds only the entries that passed validation. Building
    /// one is allowed only through the whole `ParseReport` (M4's
    /// `ArchivePlan::new`), so that what was left out is reported. This
    /// describes the manifest; it is not a build gate.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn output_name(&self) -> Option<&str> {
        self.output_name.as_deref()
    }
    /// Permissions for directory entries the archive creates.
    pub fn dir_mode(&self) -> u32 {
        self.dir_mode
    }
    /// Owner of the directory entries the archive creates.
    pub fn default_owner(&self) -> &Owner {
        &self.default_owner
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The format `output_name`'s suffix names, else `Tar`.
    pub fn default_format(&self) -> ArchiveFormat {
        self.output_name
            .as_deref()
            .and_then(|n| ArchiveFormat::from_file_name(n.as_ref()))
            .unwrap_or(ArchiveFormat::Tar)
    }
}

/// UI-facing rendering of a manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ManifestView {
    pub name: String,
    pub output_name: Option<String>,
    pub entries: Vec<EntryView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub id: String,
    pub source: String,
    /// Absolute stored name, such as `/opt/gateway/bin/gateway`.
    pub target_path: String,
    /// Four octal digits, such as `0755`.
    pub mode: String,
    /// Symbolic form, such as `rwxr-xr-x`.
    pub mode_text: String,
    /// `<uname>:<gname>`, such as `root:root`.
    pub owner: String,
    /// Numeric user id written to the header.
    pub uid: u32,
    /// Numeric group id written to the header.
    pub gid: u32,
    pub normalize_eol: bool,
}

impl From<&Manifest> for ManifestView {
    fn from(m: &Manifest) -> ManifestView {
        ManifestView {
            name: m.name.clone(),
            output_name: m.output_name.clone(),
            entries: m
                .entries
                .iter()
                .map(|e| EntryView {
                    id: e.id.clone(),
                    source: e.source.clone(),
                    target_path: e.target_path(),
                    mode: format!("{:04o}", e.mode),
                    mode_text: mode::symbolic(e.mode),
                    owner: format!("{}:{}", e.owner.uname, e.owner.gname),
                    uid: e.owner.uid,
                    gid: e.owner.gid,
                    normalize_eol: e.normalize_eol,
                })
                .collect(),
        }
    }
}
