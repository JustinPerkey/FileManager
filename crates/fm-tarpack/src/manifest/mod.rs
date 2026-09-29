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

pub use model::{Diagnostic, Entry, EntryView, Manifest, ManifestView, Owner, Severity};

#[derive(Debug)]
pub struct LoadedManifest {
    pub path: PathBuf,
    /// SHA-256 of the exact file bytes, to detect edits made outside the app.
    pub sha256: [u8; 32],
    pub manifest: Manifest,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Debug)]
pub enum LoadError {
    /// The manifest has errors. Holds every diagnostic, warnings included.
    Invalid(Vec<Diagnostic>),
    Io(io::Error),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Invalid(d) => write!(f, "manifest is invalid ({} diagnostic(s))", d.len()),
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

/// Parses and validates manifest text. On failure returns every diagnostic,
/// warnings included.
///
/// Parsing has two stages. A TOML syntax error stops at the first one, since
/// nothing past it can be read. Everything else is collected in one pass:
/// unknown keys, missing fields, wrong types and every validation rule, across
/// all entries. Within a single `[defaults]` or `[[file]]` table only the
/// first wrong-type or missing-field error is reported, and that table's
/// value rules wait until it deserializes. Errors inside a `[[file]]` table
/// name its `id`.
pub fn parse(text: &str) -> Result<(Manifest, Vec<Diagnostic>), Vec<Diagnostic>> {
    let doc = toml::de::DeTable::parse(text).map_err(|e| vec![validate::syntax_error(text, &e)])?;
    let mut ctx = validate::Ctx::new(text);
    let raw = raw::read(&mut ctx, doc.into_inner());
    validate::validate(ctx, raw)
}

pub fn load(path: &Path) -> Result<LoadedManifest, LoadError> {
    let bytes = std::fs::read(path)?;
    let sha256: [u8; 32] = Sha256::digest(&bytes).into();
    let text = match std::str::from_utf8(&bytes) {
        Ok(t) => t.strip_prefix('\u{feff}').unwrap_or(t),
        Err(e) => {
            return Err(LoadError::Invalid(vec![Diagnostic {
                severity: Severity::Error,
                line: 1,
                col: 1,
                entry_id: None,
                message: format!("the file is not valid UTF-8 (at byte {})", e.valid_up_to()),
            }]))
        }
    };
    match parse(text) {
        Ok((manifest, warnings)) => Ok(LoadedManifest {
            path: path.to_path_buf(),
            sha256,
            manifest,
            warnings,
        }),
        Err(diags) => Err(LoadError::Invalid(diags)),
    }
}

#[cfg(test)]
mod tests;
