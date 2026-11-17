//! `DbValue`: the value that crosses the boundary. Integers, reals and
//! durations travel as strings so no 64-bit number reaches TypeScript.

use rusqlite::types::{Value, ValueRef};
use serde::{Deserialize, Serialize};

use crate::rules::{duration, FieldKind, FieldRule};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum DbValue {
    Null,
    Integer(String),
    Real(String),
    Text(String),
    Duration(String),
    /// Only the length; contents never cross the boundary.
    Blob {
        #[ts(type = "number")]
        size: u64,
    },
}

impl From<ValueRef<'_>> for DbValue {
    fn from(v: ValueRef<'_>) -> Self {
        match v {
            ValueRef::Null => Self::Null,
            ValueRef::Integer(n) => Self::Integer(n.to_string()),
            ValueRef::Real(x) => Self::Real(x.to_string()),
            ValueRef::Text(t) => Self::Text(String::from_utf8_lossy(t).into_owned()),
            ValueRef::Blob(b) => Self::Blob {
                size: b.len() as u64,
            },
        }
    }
}

impl DbValue {
    /// The `SQLite` value to bind. Call only on a value `validate` returned for
    /// `rule`.
    ///
    /// # Panics
    ///
    /// When the value is not one `validate` returns for `rule`'s kind,
    /// including any blob: v1 never writes one.
    #[must_use]
    pub fn to_sql(&self, rule: &FieldRule) -> Value {
        fn bad<T>(value: &DbValue, rule: &FieldRule) -> T {
            panic!(
                "to_sql: {value:?} was not validated against {:?}",
                rule.kind
            )
        }
        match (self, rule.kind) {
            (Self::Null, _) => Value::Null,
            (Self::Integer(s), FieldKind::Integer | FieldKind::Bool) => {
                Value::Integer(s.parse().unwrap_or_else(|_| bad(self, rule)))
            }
            (Self::Real(s), FieldKind::Real) => {
                Value::Real(s.parse().unwrap_or_else(|_| bad(self, rule)))
            }
            (Self::Text(s), FieldKind::Text) => Value::Text(s.clone()),
            (Self::Duration(s), FieldKind::Duration) => {
                let unit = rule.unit.unwrap_or_else(|| bad(self, rule));
                Value::Integer(duration::parse(s, unit).unwrap_or_else(|_| bad(self, rule)))
            }
            _ => bad(self, rule),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DurationUnit;
    use crate::rules::validate;

    fn rule(kind: FieldKind) -> FieldRule {
        FieldRule {
            kind,
            nullable: true,
            unit: (kind == FieldKind::Duration).then_some(DurationUnit::Minutes),
            min: None,
            max: None,
            min_length: None,
            max_length: None,
            pattern: None,
            allowed: None,
            empty_is_null: false,
        }
    }

    #[test]
    fn db_value_json_shapes() {
        for (value, json) in [
            (DbValue::Null, r#"{"type":"null"}"#),
            (
                DbValue::Integer("42".into()),
                r#"{"type":"integer","value":"42"}"#,
            ),
            (
                DbValue::Real("1.5".into()),
                r#"{"type":"real","value":"1.5"}"#,
            ),
            (
                DbValue::Text("abc".into()),
                r#"{"type":"text","value":"abc"}"#,
            ),
            (
                DbValue::Duration("1h30m".into()),
                r#"{"type":"duration","value":"1h30m"}"#,
            ),
            (
                DbValue::Blob { size: 1024 },
                r#"{"type":"blob","value":{"size":1024}}"#,
            ),
        ] {
            assert_eq!(serde_json::to_string(&value).unwrap(), json);
            assert_eq!(serde_json::from_str::<DbValue>(json).unwrap(), value);
        }
    }

    #[test]
    fn db_value_from_storage_classes() {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open(dir.path().join("t.db")).unwrap();
        conn.execute("CREATE TABLE t (i, r, t, b, n)", []).unwrap();
        conn.execute(
            "INSERT INTO t VALUES (?1, ?2, ?3, ?4, NULL)",
            rusqlite::params![i64::MIN, 1.5_f64, "h\u{e9}llo", vec![0_u8; 1 << 20]],
        )
        .unwrap();
        let row: Vec<DbValue> = conn
            .query_row("SELECT i, r, t, b, n FROM t", [], |row| {
                (0..5)
                    .map(|i| row.get_ref(i).map(DbValue::from))
                    .collect::<Result<_, _>>()
            })
            .unwrap();
        assert_eq!(
            row,
            vec![
                DbValue::Integer(i64::MIN.to_string()),
                DbValue::Real("1.5".into()),
                DbValue::Text("h\u{e9}llo".into()),
                DbValue::Blob { size: 1_048_576 },
                DbValue::Null,
            ]
        );
        assert_eq!(
            serde_json::to_string(&row[3]).unwrap(),
            r#"{"type":"blob","value":{"size":1048576}}"#
        );
    }

    #[test]
    fn to_sql_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open(dir.path().join("t.db")).unwrap();
        conn.execute("CREATE TABLE t (x)", []).unwrap();
        let cases = [
            (
                FieldKind::Integer,
                DbValue::Integer("+007".into()),
                "integer",
                "7",
            ),
            (
                FieldKind::Bool,
                DbValue::Integer("1".into()),
                "integer",
                "1",
            ),
            (
                FieldKind::Real,
                DbValue::Real("1e3".into()),
                "real",
                "1000.0",
            ),
            (FieldKind::Text, DbValue::Text("abc".into()), "text", "abc"),
            // 1h30m in minutes
            (
                FieldKind::Duration,
                DbValue::Duration("1h30m".into()),
                "integer",
                "90",
            ),
            (FieldKind::Integer, DbValue::Null, "null", "NULL"),
        ];
        for (kind, input, ty, shown) in cases {
            let r = rule(kind);
            let ok = validate(&r, &input).unwrap();
            conn.execute("DELETE FROM t", []).unwrap();
            conn.execute(
                "INSERT INTO t VALUES (?1)",
                rusqlite::params![ok.to_sql(&r)],
            )
            .unwrap();
            let (got_ty, got): (String, String) = conn
                .query_row(
                    "SELECT typeof(x), CAST(quote(x) AS TEXT) FROM t",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(got_ty, ty, "{kind:?}");
            assert_eq!(got.trim_matches('\''), shown, "{kind:?}");
        }
    }
}
