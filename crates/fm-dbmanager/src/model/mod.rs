//! The resolved model: the introspected schema with the config laid over it.
//!
//! `resolve` never fails. Every mismatch becomes a diagnostic, and an error
//! never leaves anything more editable than its entry would have. Hidden
//! tables and fields are absent from every view and never contribute links,
//! reverse relations or checklists.

mod fields;
mod labels;
mod m2m;
mod resolve;
mod rule;
#[cfg(test)]
mod tests;
mod views;

use crate::config::{At, ConfigDiagnostic, DiagnosticSeverity, FieldMode, RelationMode, TableMode};
use crate::rules::{FieldKind, FieldRule};
use crate::schema::{Identity, Schema};
use crate::value::DbValue;

pub use resolve::resolve;
pub use views::{FieldView, ForeignKeyView, M2mView, ReverseRelationView, TableView};

/// The schema with the config applied.
#[derive(Clone, Debug)]
pub struct Model {
    schema: Schema,
    /// Parallel to `schema.tables`; `None` for a hidden table or a junction
    /// used as many-to-many that is not listed.
    tables: Vec<Option<ResolvedTable>>,
    diagnostics: Vec<ConfigDiagnostic>,
}

impl Model {
    /// Config diagnostics plus those that need the schema, sorted by line
    /// (diagnostics without a line last), then table, then column.
    #[must_use]
    pub fn diagnostics(&self) -> &[ConfigDiagnostic] {
        &self.diagnostics
    }

    /// The schema the model was resolved against.
    #[must_use]
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// The table whose SQL name is exactly `name`. `None` when it does not
    /// exist, is hidden, or is a junction used as many-to-many and not listed.
    #[must_use]
    pub fn table(&self, name: &str) -> Option<&ResolvedTable> {
        self.tables.iter().flatten().find(|t| t.name == name)
    }

    /// The resolved table at `schema_index` in `Schema::tables`; `None` when
    /// it is hidden or not listed.
    #[must_use]
    pub fn table_at(&self, schema_index: usize) -> Option<&ResolvedTable> {
        self.tables.get(schema_index)?.as_ref()
    }

    /// Every resolved table, in schema order.
    pub fn tables(&self) -> impl Iterator<Item = &ResolvedTable> {
        self.tables.iter().flatten()
    }

    /// The listed and pickable tables, in schema order. The UI leaves the
    /// pickable ones out of the table list.
    #[must_use]
    pub fn table_views(&self) -> Vec<TableView> {
        self.tables().map(views::table_view).collect()
    }
}

/// One table, resolved. Indexes are into `Schema::tables` and into that
/// table's `columns`.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedTable {
    pub schema_index: usize,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub object_name: Option<String>,
    /// `Editable`, `ReadOnly` or `Pickable`; never `Hidden`.
    pub mode: TableMode,
    pub can_create: bool,
    pub can_delete: bool,
    pub has_triggers: bool,
    pub identity: Identity,
    /// Visible fields in display order.
    pub fields: Vec<ResolvedField>,
    /// Columns the config hides; they get their column default on create.
    pub hidden_columns: Vec<usize>,
    pub label: ResolvedLabel,
    /// Label columns that are not durations.
    pub search: Vec<usize>,
    /// List order. The identity is always the final tie-break; it is not
    /// repeated here.
    pub order: Vec<OrderTerm>,
    pub referenced_by: Vec<ReverseRelation>,
    pub many_to_many: Vec<M2mSide>,
}

impl ResolvedTable {
    /// The visible field for `column`.
    #[must_use]
    pub fn field(&self, column: usize) -> Option<&ResolvedField> {
        self.fields.iter().find(|f| f.column == column)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a boundary type the UI reads field by field; the flags are independent facts"
)]
pub struct ResolvedField {
    pub column: usize,
    pub name: String,
    pub display_name: String,
    pub help: Option<String>,
    /// `Hidden` only appears on a junction's link fields, which keep their
    /// hidden ones flagged.
    pub mode: FieldMode,
    pub kind: FieldKind,
    pub declared_type: String,
    pub rule: FieldRule,
    pub is_key: bool,
    pub required_on_create: bool,
    pub has_default: bool,
    pub default_value: Option<DbValue>,
    pub multiline: bool,
    pub foreign_key: Option<ResolvedForeignKey>,
}

/// A single-column foreign key to an existing table, or the first column of a
/// multi-column one.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedForeignKey {
    /// Index into the table's `foreign_keys`.
    pub fk_index: usize,
    /// Index into `Schema::tables`.
    pub table: usize,
    pub table_name: String,
    /// Always `Some` in v1: a link exists only to a table that has a view.
    pub table_display_name: Option<String>,
    /// The referenced columns of `table`.
    pub columns: Vec<usize>,
    pub navigable: bool,
}

/// An object's title. Empty `parts` means the key label
/// (`"<display name> #<key>"`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResolvedLabel {
    pub parts: Vec<ResolvedLabelPart>,
}

impl ResolvedLabel {
    /// The columns the label reads, in order, without repeats.
    #[must_use]
    pub fn columns(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for p in &self.parts {
            if let ResolvedLabelPart::Column(c) = p {
                if !out.contains(c) {
                    out.push(*c);
                }
            }
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedLabelPart {
    Literal(String),
    Column(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderTerm {
    pub column: usize,
    pub descending: bool,
}

/// A foreign key from another table into this one.
#[derive(Clone, Debug, PartialEq)]
pub struct ReverseRelation {
    /// `fk:<table>:<fk id>`.
    pub relation: String,
    /// Index into `Schema::tables` of the referencing table.
    pub table: usize,
    pub table_name: String,
    pub table_display_name: String,
    /// Index into the referencing table's `foreign_keys`.
    pub fk_index: usize,
    /// The referencing columns.
    pub columns: Vec<usize>,
    /// Their display names, joined by `, `.
    pub via: String,
}

/// One side of a many-to-many relation.
#[derive(Clone, Debug, PartialEq)]
pub struct M2mSide {
    /// `m2m:<junction>`.
    pub relation: String,
    /// Index into `Schema::tables`.
    pub junction: usize,
    pub junction_name: String,
    pub junction_identity: Identity,
    /// The junction column that references this side's table, and the column
    /// of this table it references.
    pub this_column: usize,
    pub this_key: usize,
    /// The same for the other side.
    pub other_column: usize,
    pub other_key: usize,
    /// Index into `Schema::tables`.
    pub other_table: usize,
    pub other_table_name: String,
    pub heading: String,
    /// `Editable` or `ReadOnly` on this side.
    pub mode: RelationMode,
    /// Links can be added: every `NOT NULL` junction column without a default
    /// is a relation column or an editable or create-only link field.
    pub can_add: bool,
    /// Every link field in junction column order, hidden ones flagged by
    /// `mode == Hidden`.
    pub link_fields: Vec<ResolvedField>,
}

pub(crate) fn diag(
    severity: DiagnosticSeverity,
    message: String,
    at: Option<At>,
    table: Option<&str>,
    column: Option<&str>,
) -> ConfigDiagnostic {
    ConfigDiagnostic {
        severity,
        message,
        line: at.map(|a| a.line),
        col: at.map(|a| a.col),
        table: table.map(str::to_owned),
        column: column.map(str::to_owned),
    }
}
