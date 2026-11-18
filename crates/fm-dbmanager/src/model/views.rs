//! The per-table views the UI renders. Hidden fields are never in them.

use serde::Serialize;

use super::{M2mSide, ResolvedField, ResolvedTable};
use crate::config::{FieldMode, RelationMode, TableMode};
use crate::rules::{FieldKind, FieldRule};
use crate::value::DbValue;

#[derive(Clone, Debug, PartialEq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a boundary type the UI reads field by field; the flags are independent facts"
)]
pub struct TableView {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    /// The config's `object_name`; `None` when not configured.
    pub object_name: Option<String>,
    /// `editable`, `read-only` or `pickable`; never `hidden`. Pickable tables
    /// are not shown in the table list.
    pub mode: TableMode,
    pub can_create: bool,
    pub can_delete: bool,
    pub searchable: bool,
    pub has_triggers: bool,
    /// Display order.
    pub fields: Vec<FieldView>,
    pub referenced_by: Vec<ReverseRelationView>,
    pub many_to_many: Vec<M2mView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "a boundary type the UI reads field by field; the flags are independent facts"
)]
pub struct FieldView {
    pub column: String,
    pub display_name: String,
    pub help: Option<String>,
    /// `editable`, `read-only` or `create-only`; never `hidden`.
    pub mode: FieldMode,
    pub kind: FieldKind,
    pub declared_type: String,
    pub rule: FieldRule,
    /// Part of the primary key.
    pub is_key: bool,
    /// `NOT NULL`, no default, not a rowid alias. For a link field: required
    /// when adding a link.
    pub required_on_create: bool,
    pub has_default: bool,
    pub default_value: Option<DbValue>,
    pub multiline: bool,
    pub foreign_key: Option<ForeignKeyView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeyView {
    pub table: String,
    /// Always `Some` in v1: a link exists only to a table that has a view.
    pub table_display_name: Option<String>,
    pub navigable: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ReverseRelationView {
    /// Opaque to the UI: `fk:<table>:<fk id>`.
    pub relation: String,
    pub table: String,
    pub table_display_name: String,
    /// The referencing fields' display names, joined by `, `.
    pub via: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct M2mView {
    /// Opaque to the UI: `m2m:<junction>`.
    pub relation: String,
    pub other_table: String,
    pub heading: String,
    /// This side: `editable` or `read-only`.
    pub mode: RelationMode,
    /// Visible link fields, junction column order; `required_on_create` means
    /// required when adding a link.
    pub link_fields: Vec<FieldView>,
}

fn field_view(t: &ResolvedField) -> FieldView {
    debug_assert_ne!(t.mode, FieldMode::Hidden);
    FieldView {
        column: t.name.clone(),
        display_name: t.display_name.clone(),
        help: t.help.clone(),
        mode: t.mode,
        kind: t.kind,
        declared_type: t.declared_type.clone(),
        rule: t.rule.clone(),
        is_key: t.is_key,
        required_on_create: t.required_on_create,
        has_default: t.has_default,
        default_value: t.default_value.clone(),
        multiline: t.multiline,
        foreign_key: t.foreign_key.as_ref().map(|f| ForeignKeyView {
            table: f.table_name.clone(),
            table_display_name: f.table_display_name.clone(),
            navigable: f.navigable,
        }),
    }
}

fn m2m_view(s: &M2mSide) -> M2mView {
    M2mView {
        relation: s.relation.clone(),
        other_table: s.other_table_name.clone(),
        heading: s.heading.clone(),
        mode: s.mode,
        link_fields: s
            .link_fields
            .iter()
            .filter(|f| f.mode != FieldMode::Hidden)
            .map(field_view)
            .collect(),
    }
}

pub(super) fn table_view(t: &ResolvedTable) -> TableView {
    TableView {
        name: t.name.clone(),
        display_name: t.display_name.clone(),
        description: t.description.clone(),
        object_name: t.object_name.clone(),
        mode: t.mode,
        can_create: t.can_create,
        can_delete: t.can_delete,
        searchable: !t.search.is_empty(),
        has_triggers: t.has_triggers,
        fields: t.fields.iter().map(field_view).collect(),
        referenced_by: t
            .referenced_by
            .iter()
            .map(|r| ReverseRelationView {
                relation: r.relation.clone(),
                table: r.table_name.clone(),
                table_display_name: r.table_display_name.clone(),
                via: r.via.clone(),
            })
            .collect(),
        many_to_many: t.many_to_many.iter().map(m2m_view).collect(),
    }
}
