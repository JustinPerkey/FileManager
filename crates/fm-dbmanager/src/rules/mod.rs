//! Field kinds and rules, durations, and the pure, authoritative `validate`.
//! The TypeScript mirror of `validate` must pass the same vectors
//! (`examples/dbmanager/validation-cases.json`).

pub mod duration;
mod validate;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::config::DurationUnit;
use crate::value::DbValue;

pub use validate::validate;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum FieldKind {
    Integer,
    Real,
    Text,
    Bool,
    Duration,
    Blob,
    Any,
}

/// One value a field may take, with the label the UI shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct AllowedValue {
    pub value: DbValue,
    pub label: String,
}

/// What a field accepts. Plain data: resolving it from config and schema is
/// done elsewhere.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct FieldRule {
    pub kind: FieldKind,
    pub nullable: bool,
    /// `Some` iff `kind` is `Duration`.
    pub unit: Option<DurationUnit>,
    /// Integer/real: canonical decimal. Duration: canonical duration text.
    pub min: Option<String>,
    pub max: Option<String>,
    pub min_length: Option<u32>,
    pub max_length: Option<u32>,
    /// Unanchored source, as written in the config.
    pub pattern: Option<String>,
    pub allowed: Option<Vec<AllowedValue>>,
    pub empty_is_null: bool,
}

/// Declaration order is the order `validate` reports issues in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum FieldIssueCode {
    Required,
    WrongType,
    NotInteger,
    NotReal,
    NotBool,
    NotDuration,
    DurationPrecision,
    BelowMin,
    AboveMax,
    TooShort,
    TooLong,
    PatternMismatch,
    NotAllowed,
    // Database-dependent: never produced by `validate`.
    Unique,
    ReferenceMissing,
    NotEditable,
    UnknownField,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct FieldIssue {
    pub code: FieldIssueCode,
    /// An English sentence for logs and fallback.
    pub message: String,
    /// The violated bound as text, when there is one.
    pub limit: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct FieldError {
    pub column: String,
    pub issues: Vec<FieldIssue>,
}
