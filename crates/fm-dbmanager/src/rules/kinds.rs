//! Kind inference from a column's declared type, the `type` override rules,
//! and reading a column default as a literal.

use super::duration;
use super::FieldKind;
use crate::config::{DurationUnit, FieldType};
use crate::schema::{Affinity, Column};
use crate::value::DbValue;

/// The declared type, upper-cased and trimmed, with any `(...)` suffix removed.
fn normalized(declared: &str) -> String {
    declared
        .split('(')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase()
}

/// The kind a column has when the config does not say: the doc's seven
/// ordered rules. `strict` columns have no `BOOL`/`BOOLEAN` declared type
/// (`SQLite` refuses to create them), so rule 1 is skipped there. `duration`
/// is never inferred.
#[must_use]
pub fn infer_kind(column: &Column, strict: bool) -> FieldKind {
    let t = normalized(&column.declared_type);
    if !strict && (t == "BOOLEAN" || t == "BOOL") {
        FieldKind::Bool
    } else if t.is_empty() || t == "ANY" {
        FieldKind::Any
    } else if t.contains("INT") {
        FieldKind::Integer
    } else if t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT") {
        FieldKind::Text
    } else if t.contains("BLOB") {
        FieldKind::Blob
    } else {
        // REAL, FLOA, DOUB, and everything else (NUMERIC affinity).
        FieldKind::Real
    }
}

#[must_use]
pub(crate) fn kind_of_type(ty: FieldType) -> FieldKind {
    match ty {
        FieldType::Integer => FieldKind::Integer,
        FieldType::Real => FieldKind::Real,
        FieldType::Text => FieldKind::Text,
        FieldType::Bool => FieldKind::Bool,
        FieldType::Duration => FieldKind::Duration,
    }
}

/// Whether `type = ty` is allowed on `column`: only where the column's
/// affinity stores the kind faithfully.
#[must_use]
pub(crate) fn type_override_allowed(column: &Column, strict: bool, ty: FieldType) -> bool {
    let t = normalized(&column.declared_type);
    if strict {
        return match t.as_str() {
            "INT" | "INTEGER" => matches!(
                ty,
                FieldType::Integer | FieldType::Bool | FieldType::Duration
            ),
            "REAL" => ty == FieldType::Real,
            "TEXT" => ty == FieldType::Text,
            "ANY" => true,
            _ => false,
        };
    }
    let untyped = t.is_empty() || t == "ANY";
    let a = column.affinity;
    match ty {
        FieldType::Integer | FieldType::Bool | FieldType::Duration => {
            untyped || matches!(a, Affinity::Integer | Affinity::Numeric)
        }
        FieldType::Real => untyped || matches!(a, Affinity::Real | Affinity::Numeric),
        FieldType::Text => untyped || matches!(a, Affinity::Text | Affinity::Numeric),
    }
}

/// Reads a `DEFAULT` clause as a literal: a signed integer, a real, a
/// single-quoted string (`''` escapes), `NULL`, `TRUE` or `FALSE`. `None`
/// means the default exists but its value is not known (`CURRENT_TIMESTAMP`,
/// an expression). For a duration field an integer literal is a count of
/// `unit`.
#[must_use]
pub(crate) fn parse_default(
    sql: &str,
    kind: FieldKind,
    unit: Option<DurationUnit>,
) -> Option<DbValue> {
    let s = sql.trim();
    if s.eq_ignore_ascii_case("NULL") {
        return Some(DbValue::Null);
    }
    if s.eq_ignore_ascii_case("TRUE") {
        return Some(DbValue::Integer("1".into()));
    }
    if s.eq_ignore_ascii_case("FALSE") {
        return Some(DbValue::Integer("0".into()));
    }
    if let Some(inner) = s.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        // Every quote inside must be doubled.
        let mut chars = inner.chars().peekable();
        let mut out = String::new();
        while let Some(c) = chars.next() {
            if c == '\'' && chars.next() != Some('\'') {
                return None;
            }
            out.push(c);
        }
        return Some(DbValue::Text(out));
    }
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        let n: i64 = s.parse().ok()?;
        if kind == FieldKind::Duration {
            return Some(DbValue::Duration(duration::format(n, unit?)));
        }
        return Some(DbValue::Integer(n.to_string()));
    }
    super::validate::parse_real(s).map(|_| DbValue::Real(s.to_string()))
}
