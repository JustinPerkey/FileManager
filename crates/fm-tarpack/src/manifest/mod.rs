//! Manifest parsing and validation. The format is documented in
//! `docs/tarpack-manifest.md`.

mod mode;
mod model;
mod raw;
mod validate;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub use model::{
    Diagnostic, Entry, EntryFailure, EntryView, Manifest, ManifestView, Owner, ParseReport,
    Severity,
};

#[derive(Debug)]
pub struct LoadedManifest {
    pub path: PathBuf,
    /// SHA-256 of the exact file bytes, to detect edits made outside the app.
    pub sha256: [u8; 32],
    pub report: ParseReport,
}

#[derive(Debug)]
pub enum LoadError {
    Io(io::Error),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "cannot read manifest: {e}"),
        }
    }
}

impl std::error::Error for LoadError {}

impl From<io::Error> for LoadError {
    fn from(e: io::Error) -> Self {
        LoadError::Io(e)
    }
}

/// Parses and validates manifest text into a report. Errors do not stop the
/// parse: the report holds the entries that passed, one failure per `[[file]]`
/// table that did not, the manifest-level errors, and the warnings.
///
/// A TOML syntax error stops at the first one, since nothing past it can be
/// read; the report then holds no entries. Everything else is collected in one
/// pass: unknown keys, missing fields, wrong types and every validation rule,
/// across all entries. Within a single `[defaults]` or `[[file]]` table only
/// the first wrong-type or missing-field error is reported, and that table's
/// value rules wait until it deserializes. Errors inside a `[[file]]` table
/// name its `id`, or its position when it has none. Errors in `[defaults]`,
/// `version`, an unknown top-level key, or `file` withhold every entry.
pub fn parse(text: &str) -> ParseReport {
    let doc = match toml::de::DeTable::parse(text) {
        Ok(doc) => doc,
        Err(e) => return validate::withheld_report(validate::syntax_error(text, &e)),
    };
    let mut ctx = validate::Ctx::new(text);
    let raw = raw::read(&mut ctx, doc.into_inner());
    validate::validate(ctx, raw)
}

/// Reads and parses a manifest file. A readable file always loads, whatever
/// its errors.
pub fn load(path: &Path) -> Result<LoadedManifest, LoadError> {
    let bytes = std::fs::read(path)?;
    let sha256: [u8; 32] = Sha256::digest(&bytes).into();
    let report = match std::str::from_utf8(&bytes) {
        Ok(t) => parse(t.strip_prefix('\u{feff}').unwrap_or(t)),
        Err(e) => validate::withheld_report(Diagnostic {
            severity: Severity::Error,
            line: 1,
            col: 1,
            entry_id: None,
            message: format!("the file is not valid UTF-8 (at byte {})", e.valid_up_to()),
        }),
    };
    Ok(LoadedManifest {
        path: path.to_path_buf(),
        sha256,
        report,
    })
}

#[cfg(test)]
mod tests;
