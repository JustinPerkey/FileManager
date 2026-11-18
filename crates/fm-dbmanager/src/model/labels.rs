//! Labels, search columns, list order and field order.

use super::{diag, OrderTerm, ResolvedField, ResolvedLabel, ResolvedLabelPart};
use crate::config::{ConfigDiagnostic, DiagnosticSeverity, FieldMode, LabelPart, TableConfig};
use crate::rules::FieldKind;
use crate::schema::Table;

fn visible(fields: &[ResolvedField], column: usize) -> bool {
    fields[column].mode != FieldMode::Hidden
}

fn error(diags: &mut Vec<ConfigDiagnostic>, table: &Table, at: crate::config::At, msg: &str) {
    diags.push(diag(
        DiagnosticSeverity::Error,
        format!("table `{}`: {msg}", table.name),
        Some(at),
        Some(table.name.as_str()),
        None,
    ));
}

/// The default label: the first visible text column named `name`, `title` or
/// `label`. Empty when there is none (the key label).
fn default_label(table: &Table, fields: &[ResolvedField]) -> ResolvedLabel {
    for wanted in ["name", "title", "label"] {
        let found = table
            .columns
            .iter()
            .position(|c| c.name.as_str().eq_ignore_ascii_case(wanted));
        if let Some(i) = found {
            if fields[i].kind == FieldKind::Text && visible(fields, i) {
                return ResolvedLabel {
                    parts: vec![ResolvedLabelPart::Column(i)],
                };
            }
        }
    }
    ResolvedLabel::default()
}

/// `fields` has one entry per column of `table`, hidden ones included.
pub(super) fn resolve_label(
    table: &Table,
    cfg: Option<&TableConfig>,
    fields: &[ResolvedField],
    diags: &mut Vec<ConfigDiagnostic>,
) -> ResolvedLabel {
    let Some(label) = cfg.and_then(|c| c.label.as_ref()) else {
        return default_label(table, fields);
    };
    let mut parts = Vec::new();
    for part in &label.value.parts {
        match part {
            LabelPart::Literal(s) => parts.push(ResolvedLabelPart::Literal(s.clone())),
            LabelPart::Column(name) => match table.column(name) {
                Some(i) if visible(fields, i) => parts.push(ResolvedLabelPart::Column(i)),
                _ => {
                    error(
                        diags,
                        table,
                        label.at,
                        &format!(
                            "`label` names `{name}`, which is not a visible column; the default label is used"
                        ),
                    );
                    return default_label(table, fields);
                }
            },
        }
    }
    ResolvedLabel { parts }
}

/// The label's columns that are not durations.
pub(super) fn search_columns(label: &ResolvedLabel, fields: &[ResolvedField]) -> Vec<usize> {
    label
        .columns()
        .into_iter()
        .filter(|&c| fields[c].kind != FieldKind::Duration)
        .collect()
}

/// `order_by`, or the label columns ascending.
pub(super) fn resolve_order(
    table: &Table,
    cfg: Option<&TableConfig>,
    fields: &[ResolvedField],
    label: &ResolvedLabel,
    diags: &mut Vec<ConfigDiagnostic>,
) -> Vec<OrderTerm> {
    let default = || {
        label
            .columns()
            .into_iter()
            .map(|column| OrderTerm {
                column,
                descending: false,
            })
            .collect()
    };
    let Some(cfg) = cfg.filter(|c| !c.order_by.is_empty()) else {
        return default();
    };
    let mut out = Vec::new();
    let mut ok = true;
    for o in &cfg.order_by {
        match table.column(&o.column) {
            Some(i) if visible(fields, i) => out.push(OrderTerm {
                column: i,
                descending: o.descending,
            }),
            _ => {
                ok = false;
                error(
                    diags,
                    table,
                    o.at,
                    &format!(
                        "`order_by` names `{}`, which is not a visible column; the default order is used",
                        o.column
                    ),
                );
            }
        }
    }
    if ok {
        out
    } else {
        default()
    }
}

/// Visible columns in display order: `field_order` first, the rest in column
/// order. An unknown or hidden name ignores the whole key.
pub(super) fn display_order(
    table: &Table,
    cfg: Option<&TableConfig>,
    fields: &[ResolvedField],
    diags: &mut Vec<ConfigDiagnostic>,
) -> Vec<usize> {
    let mut first: Vec<usize> = Vec::new();
    let mut ok = true;
    if let Some(cfg) = cfg {
        for name in &cfg.field_order {
            match table.column(&name.value) {
                Some(i) if visible(fields, i) => {
                    if !first.contains(&i) {
                        first.push(i);
                    }
                }
                _ => {
                    ok = false;
                    error(
                        diags,
                        table,
                        name.at,
                        &format!(
                            "`field_order` names `{}`, which is not a visible column; the column order is used",
                            name.value
                        ),
                    );
                }
            }
        }
    }
    if !ok {
        first.clear();
    }
    let rest: Vec<usize> = (0..table.columns.len())
        .filter(|&i| visible(fields, i) && !first.contains(&i))
        .collect();
    first.extend(rest);
    first
}
