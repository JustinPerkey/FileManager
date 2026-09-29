//! Serde structs mirroring the TOML. Spans are kept on the fields that need a
//! location in a diagnostic.

use serde::Deserialize;
use toml::Spanned;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawManifest {
    pub version: Spanned<i64>,
    pub name: Spanned<String>,
    pub output_name: Option<Spanned<String>>,
    pub defaults: Option<RawDefaults>,
    #[serde(default)]
    pub file: Vec<Spanned<RawFile>>,
}

/// `normalize_eol` is deliberately absent: it is a per-file choice.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDefaults {
    pub mode: Option<Spanned<String>>,
    pub dir_mode: Option<Spanned<String>>,
    pub uid: Option<Spanned<i64>>,
    pub gid: Option<Spanned<i64>>,
    pub uname: Option<Spanned<String>>,
    pub gname: Option<Spanned<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawFile {
    pub id: Spanned<String>,
    pub source: Spanned<String>,
    pub dir: Spanned<String>,
    pub name: Option<Spanned<String>>,
    pub mode: Option<Spanned<String>>,
    pub uid: Option<Spanned<i64>>,
    pub gid: Option<Spanned<i64>>,
    pub uname: Option<Spanned<String>>,
    pub gname: Option<Spanned<String>>,
    pub normalize_eol: Option<Spanned<bool>>,
}
