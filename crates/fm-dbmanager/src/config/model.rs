//! The parsed Database Manager config. Everything here is checked from the
//! config text alone; matching names against the database is a later step.

use serde::Serialize;

/// 1-based line and column (in characters) of a key in the config text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct At {
    pub line: u32,
    pub col: u32,
}

/// A setting that was written in the config, with the position of its key.
/// An absent setting is `None`, which is how explicit and inherited settings
/// are told apart.
#[derive(Clone, Debug, PartialEq)]
pub struct Located<T> {
    pub value: T,
    pub at: At,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// A problem found in the config. `line` and `col` are 1-based; `table` and
/// `column` are for diagnostics that come from the database instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub line: Option<u32>,
    pub col: Option<u32>,
    pub table: Option<String>,
    pub column: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "kebab-case")]
pub enum TableMode {
    Editable,
    ReadOnly,
    Hidden,
    /// Not listed, but offered in pickers and as links; nothing in it
    /// changes. Never valid in `[defaults]`.
    Pickable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "kebab-case")]
pub enum FieldMode {
    Editable,
    ReadOnly,
    Hidden,
    CreateOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "kebab-case")]
pub enum RelationMode {
    Editable,
    ReadOnly,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "lowercase")]
pub enum DurationUnit {
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
    Days,
}

/// The `type` key: overrides the kind inferred from the column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldType {
    Integer,
    Real,
    Text,
    Bool,
    Duration,
}

impl TableMode {
    pub(super) fn parse(s: &str) -> Option<Self> {
        match s {
            "editable" => Some(Self::Editable),
            "read-only" => Some(Self::ReadOnly),
            "hidden" => Some(Self::Hidden),
            "pickable" => Some(Self::Pickable),
            _ => None,
        }
    }
}

impl FieldMode {
    pub(super) fn parse(s: &str) -> Option<Self> {
        match s {
            "editable" => Some(Self::Editable),
            "read-only" => Some(Self::ReadOnly),
            "hidden" => Some(Self::Hidden),
            "create-only" => Some(Self::CreateOnly),
            _ => None,
        }
    }
}

impl RelationMode {
    pub(super) fn parse(s: &str) -> Option<Self> {
        match s {
            "editable" => Some(Self::Editable),
            "read-only" => Some(Self::ReadOnly),
            "hidden" => Some(Self::Hidden),
            _ => None,
        }
    }
}

impl DurationUnit {
    pub(super) fn parse(s: &str) -> Option<Self> {
        match s {
            "milliseconds" => Some(Self::Milliseconds),
            "seconds" => Some(Self::Seconds),
            "minutes" => Some(Self::Minutes),
            "hours" => Some(Self::Hours),
            "days" => Some(Self::Days),
            _ => None,
        }
    }
}

impl FieldType {
    pub(super) fn parse(s: &str) -> Option<Self> {
        match s {
            "integer" => Some(Self::Integer),
            "real" => Some(Self::Real),
            "text" => Some(Self::Text),
            "bool" => Some(Self::Bool),
            "duration" => Some(Self::Duration),
            _ => None,
        }
    }
}

/// The modes applied to everything the config does not mention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Defaults {
    pub table_mode: TableMode,
    pub field_mode: FieldMode,
    pub relation_mode: RelationMode,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            table_mode: TableMode::Editable,
            field_mode: FieldMode::Editable,
            relation_mode: RelationMode::Editable,
        }
    }
}

impl Defaults {
    pub(super) fn read_only() -> Self {
        Defaults {
            table_mode: TableMode::ReadOnly,
            field_mode: FieldMode::ReadOnly,
            relation_mode: RelationMode::ReadOnly,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub defaults: Defaults,
    pub tables: Vec<TableEntry>,
    pub many_to_many: Vec<M2mEntry>,
}

impl Default for Config {
    /// No entries, built-in defaults: everything editable.
    fn default() -> Self {
        Config {
            defaults: Defaults::default(),
            tables: Vec::new(),
            many_to_many: Vec::new(),
        }
    }
}

impl Config {
    /// No entries, every default `read-only`. Used when the config's intent
    /// cannot be known (syntax error, unsupported version, unreadable file).
    #[must_use]
    pub fn fail_closed() -> Self {
        Config {
            defaults: Defaults::read_only(),
            tables: Vec::new(),
            many_to_many: Vec::new(),
        }
    }
}

/// An entry with an error is `Rejected`: its subject becomes read-only, or
/// hidden when the entry's own valid `mode` said `hidden`.
#[derive(Clone, Debug, PartialEq)]
pub enum TableEntry {
    Valid(TableConfig),
    /// `fields` are the table's field entries as parsed, valid or rejected.
    /// They only keep hidden fields hidden; every other effect is dropped.
    Rejected {
        name: String,
        at: At,
        hidden: bool,
        fields: Vec<FieldEntry>,
    },
}

#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "a Valid entry is the common case and is never moved around; boxing it would only add noise"
)]
pub enum FieldEntry {
    Valid(FieldConfig),
    Rejected { name: String, at: At, hidden: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub enum M2mEntry {
    Valid(M2mConfig),
    Rejected { name: String, at: At },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableConfig {
    pub name: String,
    pub at: At,
    pub mode: Option<Located<TableMode>>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub object_name: Option<String>,
    pub label: Option<Located<LabelTemplate>>,
    /// Empty when `order_by` is absent.
    pub order_by: Vec<OrderBy>,
    /// Empty when `field_order` is absent.
    pub field_order: Vec<Located<String>>,
    pub allow_create: Option<Located<bool>>,
    pub allow_delete: Option<Located<bool>>,
    pub fields: Vec<FieldEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrderBy {
    pub column: String,
    pub descending: bool,
    pub at: At,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldConfig {
    pub name: String,
    pub at: At,
    pub mode: Option<Located<FieldMode>>,
    pub display_name: Option<String>,
    pub help: Option<String>,
    pub field_type: Option<Located<FieldType>>,
    pub unit: Option<Located<DurationUnit>>,
    pub nullable: Option<Located<bool>>,
    pub min: Option<Located<Bound>>,
    pub max: Option<Located<Bound>>,
    pub min_length: Option<Located<u32>>,
    pub max_length: Option<Located<u32>>,
    pub pattern: Option<Located<String>>,
    pub allowed: Option<Located<AllowedValues>>,
    pub empty_is_null: Option<Located<bool>>,
    pub multiline: Option<Located<bool>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct M2mConfig {
    pub name: String,
    pub at: At,
    pub enabled: Option<Located<bool>>,
    pub mode: Option<Located<RelationMode>>,
    pub columns: Option<Located<[String; 2]>>,
    pub headings: Vec<Heading>,
}

/// The checklist heading shown on `side`'s objects.
#[derive(Clone, Debug, PartialEq)]
pub struct Heading {
    pub side: String,
    pub text: String,
    pub at: At,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LabelTemplate {
    pub parts: Vec<LabelPart>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabelPart {
    Literal(String),
    Column(String),
}

/// A `min` or `max`. `Duration` holds the human text as written; it matched
/// the duration grammar, but whether it is a whole number of the field's unit
/// needs the resolved unit.
#[derive(Clone, Debug, PartialEq)]
pub enum Bound {
    Int(i64),
    Float(f64),
    Duration(String),
}

/// Each value with its optional label.
#[derive(Clone, Debug, PartialEq)]
pub enum AllowedValues {
    Integers(Vec<(i64, Option<String>)>),
    Texts(Vec<(String, Option<String>)>),
}
