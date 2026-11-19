//! Keys: what addresses a row, and how a key turns back into conditions.

use rusqlite::types::Value;

use super::cell::Row;
use super::types::ObjectKey;
use crate::schema::{Ident, Identity, Table};
use crate::value::DbValue;

fn bind(value: &DbValue) -> Option<Value> {
    match value {
        DbValue::Integer(s) => s.parse().ok().map(Value::Integer),
        DbValue::Real(s) => s.parse().ok().map(Value::Real),
        DbValue::Text(s) => Some(Value::Text(s.clone())),
        DbValue::Null | DbValue::Duration(_) | DbValue::Blob { .. } => None,
    }
}

/// The equality conditions that find the row `key` names. `None` when the
/// key's shape does not fit the table.
pub(super) fn conditions<'a>(table: &'a Table, key: &ObjectKey) -> Option<Vec<(&'a Ident, Value)>> {
    match (&table.identity, key) {
        (Identity::Rowid { accessor, .. }, ObjectKey::Rowid { value }) => {
            let n = value.parse::<i64>().ok()?;
            Some(vec![(accessor, Value::Integer(n))])
        }
        (Identity::PrimaryKey { columns }, ObjectKey::Primary { values })
            if columns.len() == values.len() =>
        {
            columns
                .iter()
                .zip(values)
                .map(|(&c, v)| Some((&table.columns[c].name, bind(v)?)))
                .collect()
        }
        _ => None,
    }
}

/// The columns, in key order, that identify a row; for a rowid table the
/// accessor stands in for them.
pub(super) fn identity_idents(table: &Table) -> Vec<&Ident> {
    match &table.identity {
        Identity::Rowid { accessor, .. } => vec![accessor],
        Identity::PrimaryKey { columns } => {
            columns.iter().map(|&c| &table.columns[c].name).collect()
        }
    }
}

/// The key of a row that was selected with its identity.
pub(super) fn key_of(table: &Table, row: &Row) -> ObjectKey {
    match &table.identity {
        Identity::Rowid { .. } => ObjectKey::Rowid {
            value: row.rowid.unwrap_or_default().to_string(),
        },
        Identity::PrimaryKey { columns } => ObjectKey::Primary {
            values: columns.iter().map(|&c| row.cell(c).db_value()).collect(),
        },
    }
}

/// The key as the fallback label shows it.
pub(super) fn key_text(key: &ObjectKey) -> String {
    match key {
        ObjectKey::Rowid { value } => value.clone(),
        ObjectKey::Primary { values } => values
            .iter()
            .map(|v| match v {
                DbValue::Integer(s)
                | DbValue::Real(s)
                | DbValue::Text(s)
                | DbValue::Duration(s) => s.as_str(),
                DbValue::Null | DbValue::Blob { .. } => "",
            })
            .collect::<Vec<_>>()
            .join(", "),
    }
}
