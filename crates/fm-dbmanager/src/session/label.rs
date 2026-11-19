//! Labels and field values as the session reads them.

use super::cell::{Cell, Row};
use super::key::key_text;
use super::types::{ObjectKey, ValueProblem};
use crate::model::{ResolvedField, ResolvedLabelPart, ResolvedTable};
use crate::rules::{duration, validate, FieldIssueCode, FieldKind};
use crate::schema::Identity;
use crate::value::DbValue;

/// A cell as label text.
fn text_of(field: Option<&ResolvedField>, cell: &Cell) -> String {
    let kind = field.map(|f| f.kind);
    match cell {
        Cell::Null | Cell::Blob(_) => String::new(),
        Cell::Integer(n) => match (kind, field.and_then(|f| f.rule.unit)) {
            (Some(FieldKind::Bool), _) if *n == 0 || *n == 1 => (*n == 1).to_string(),
            (Some(FieldKind::Duration), Some(unit)) => duration::format(*n, unit),
            _ => n.to_string(),
        },
        Cell::Real(x) => x.to_string(),
        Cell::Text(t) => String::from_utf8_lossy(t).into_owned(),
    }
}

/// The table's label for a row, or `"<display name> #<key>"`.
pub(super) fn render(table: &ResolvedTable, key: &ObjectKey, row: &Row) -> String {
    let mut out = String::new();
    for part in &table.label.parts {
        match part {
            ResolvedLabelPart::Literal(s) => out.push_str(s),
            ResolvedLabelPart::Column(c) => {
                out.push_str(&text_of(table.field(*c), row.cell(*c)));
            }
        }
    }
    if !out.is_empty() {
        out
    } else if identity_hidden(table) {
        table.display_name.clone()
    } else {
        format!("{} #{}", table.display_name, key_text(key))
    }
}

/// True if any identity column is hidden, so the key must not show in a label.
/// A rowid without an alias column is not a column and cannot be hidden.
fn identity_hidden(table: &ResolvedTable) -> bool {
    let hidden = |c: &usize| table.hidden_columns.contains(c);
    match &table.identity {
        Identity::Rowid { alias_column, .. } => alias_column.iter().any(hidden),
        Identity::PrimaryKey { columns } => columns.iter().any(hidden),
    }
}

fn fits(kind: FieldKind, cell: &Cell) -> bool {
    match (kind, cell) {
        (_, Cell::Null)
        | (FieldKind::Blob | FieldKind::Any, _)
        | (
            FieldKind::Integer | FieldKind::Bool | FieldKind::Duration | FieldKind::Real,
            Cell::Integer(_),
        )
        | (FieldKind::Text, Cell::Text(_)) => true,
        (FieldKind::Real, Cell::Real(x)) => x.is_finite(),
        _ => false,
    }
}

const BOUND_CODES: [FieldIssueCode; 7] = [
    FieldIssueCode::BelowMin,
    FieldIssueCode::AboveMax,
    FieldIssueCode::TooShort,
    FieldIssueCode::TooLong,
    FieldIssueCode::PatternMismatch,
    FieldIssueCode::NotAllowed,
    FieldIssueCode::Required,
];

/// A stored value in the form `validate` takes.
fn as_input(field: &ResolvedField, cell: &Cell, shown: &DbValue) -> DbValue {
    match (field.kind, cell) {
        (FieldKind::Real, Cell::Integer(n)) => DbValue::Real(n.to_string()),
        _ => shown.clone(),
    }
}

/// The value as shown, whether a bool is ticked, and the one problem it has.
pub(super) fn read_value(
    field: &ResolvedField,
    cell: &Cell,
) -> (DbValue, Option<bool>, Option<ValueProblem>) {
    let shown = match (field.kind, cell, field.rule.unit) {
        (FieldKind::Duration, Cell::Integer(n), Some(unit)) => {
            DbValue::Duration(duration::format(*n, unit))
        }
        _ => cell.db_value(),
    };
    let problem = if !fits(field.kind, cell) {
        Some(ValueProblem::WrongType)
    } else if field.kind == FieldKind::Bool
        && matches!(cell, Cell::Integer(n) if *n != 0 && *n != 1)
    {
        Some(ValueProblem::NotBool)
    } else if matches!(
        field.kind,
        FieldKind::Integer
            | FieldKind::Real
            | FieldKind::Text
            | FieldKind::Bool
            | FieldKind::Duration
    ) && validate(&field.rule, &as_input(field, cell, &shown))
        .is_err_and(|issues| issues.iter().any(|i| BOUND_CODES.contains(&i.code)))
    {
        Some(ValueProblem::OutOfBounds)
    } else {
        None
    };
    let checked = match (field.kind, cell, problem) {
        (FieldKind::Bool, Cell::Integer(n), None) => Some(*n == 1),
        _ => None,
    };
    (shown, checked, problem)
}
