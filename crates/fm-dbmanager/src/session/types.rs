//! Boundary types of the session's reads.

use serde::{Deserialize, Serialize};

use crate::rules::{FieldError, FieldIssue};
use crate::value::DbValue;

/// Opaque. May carry hidden identity values. Never render it.
///
/// A rowid table is keyed by its rowid (a decimal string);
/// a `WITHOUT ROWID` table by its primary-key values in key order.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ObjectKey {
    Rowid { value: String },
    Primary { values: Vec<DbValue> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ReferencingFilter {
    pub relation: String,
    pub key: ObjectKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub search: Option<String>,
    pub offset: u32,
    pub limit: u32,
    pub referencing: Option<ReferencingFilter>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ObjectSummary {
    pub key: ObjectKey,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPage {
    pub items: Vec<ObjectSummary>,
    #[ts(type = "number")]
    pub total: u64,
    pub offset: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum ValueProblem {
    WrongType,
    NotBool,
    OutOfBounds,
    DanglingReference,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkTarget {
    pub table: String,
    pub key: Option<ObjectKey>,
    pub label: Option<String>,
    pub navigable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct FieldValue {
    pub column: String,
    pub value: DbValue,
    /// Bool kind: `Some` for 0 or 1; `None` for NULL or a problem.
    pub checked: Option<bool>,
    /// FK fields with a non-NULL value.
    pub link: Option<LinkTarget>,
    pub problem: Option<ValueProblem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct RelationCount {
    pub relation: String,
    #[ts(type = "number")]
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ObjectView {
    pub table: String,
    pub key: ObjectKey,
    pub label: String,
    pub version: String,
    /// `TableView.fields` order; visible only.
    pub fields: Vec<FieldValue>,
    /// One per reverse relation, same order.
    pub referenced_by: Vec<RelationCount>,
    /// One per many-to-many side: the linked count.
    pub many_to_many: Vec<RelationCount>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct M2mQuery {
    pub search: Option<String>,
    pub offset: u32,
    pub limit: u32,
    pub linked_only: bool,
    /// Fetch exactly this one other-side object: the page holds it, or
    /// nothing if it no longer exists. `search`, `offset` and `linked_only`
    /// are ignored; `total` is 0 or 1; `linked_total` is still the
    /// relation's full linked count.
    #[serde(default)]
    #[ts(optional)]
    pub only: Option<ObjectKey>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct M2mOption {
    pub key: ObjectKey,
    pub label: String,
    pub linked: bool,
    /// Linked options only: the visible link fields, in link-field order.
    pub link_values: Option<Vec<FieldValue>>,
    /// Linked options only: the junction row's version.
    pub link_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct M2mPage {
    pub items: Vec<M2mOption>,
    #[ts(type = "number")]
    pub total: u64,
    #[ts(type = "number")]
    pub linked_total: u64,
    pub offset: u32,
}

/// One save: the changed fields of one object, or a new object's fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveRequest {
    pub table: String,
    /// `None` creates an object.
    pub key: Option<ObjectKey>,
    /// Required when `key` is `Some`; a missing one never matches.
    pub version: Option<String>,
    /// The config hash the client's draft was checked against
    /// (`LoadedConfig::hash`). A save prepared under old rules is refused.
    pub config_hash: String,
    pub changes: Vec<FieldChange>,
    /// Many-to-many link changes, committed in the same transaction. Always
    /// present on the TypeScript side (send `[]` for none); a missing JSON
    /// array deserialises as empty.
    #[serde(default)]
    pub links: Vec<LinkChange>,
}

/// The staged changes to one many-to-many relation of the saved object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkChange {
    /// `m2m:<junction>`, a many-to-many side of the saved table.
    pub relation: String,
    #[serde(default)]
    pub add: Vec<LinkAdd>,
    #[serde(default)]
    pub update: Vec<LinkUpdate>,
    #[serde(default)]
    pub remove: Vec<LinkRemove>,
}

/// A new link. Link fields left out take their column default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkAdd {
    /// The object on the other side.
    pub other: ObjectKey,
    #[serde(default)]
    pub fields: Vec<FieldChange>,
}

/// New values for an existing link's fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkUpdate {
    pub other: ObjectKey,
    /// The `link_version` the UI read; a different stored one is a conflict.
    pub version: String,
    pub fields: Vec<FieldChange>,
}

/// A link to delete.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkRemove {
    pub other: ObjectKey,
    /// The `link_version` the UI read. `None` removes whatever is stored.
    pub version: Option<String>,
}

/// One link of one relation of the saved object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkRef {
    pub relation: String,
    pub other: ObjectKey,
}

/// What is wrong with one staged link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkError {
    pub relation: String,
    pub other: ObjectKey,
    /// Issues on link fields.
    pub fields: Vec<FieldError>,
    /// Issues on the link itself (`referenceMissing`, `notAllowed`).
    pub issues: Vec<FieldIssue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    pub column: String,
    pub input: FieldInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FieldInput {
    /// A plain field: a bool is the integer `"0"` or `"1"`, a duration is
    /// `{"type":"duration","value":"1h30m"}`.
    Value { value: DbValue },
    /// A foreign-key field: the picked object, or `None` for NULL.
    Reference { key: Option<ObjectKey> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    /// The object as stored after the save (triggers may have changed it).
    pub object: ObjectView,
    /// Display path of the backup this save created; `None` when an earlier
    /// write of the session already made one.
    pub backup_created: Option<String>,
}
