//! Schema introspection. Names are only ever bound as parameters to the
//! table-valued pragma functions, never interpolated into SQL.

mod ident;
mod model;
#[cfg(test)]
mod tests;

use crate::db::Database;
use rusqlite::Connection;
use std::collections::HashMap;

pub use ident::Ident;
pub use model::{
    Affinity, Column, FkAction, FkTarget, ForeignKey, Identity, Schema, SkipReason, Skipped, Table,
};

const ROWID_NAMES: [&str; 3] = ["rowid", "_rowid_", "oid"];

struct ListEntry {
    schema: String,
    name: String,
    kind: String,
    without_rowid: bool,
    strict: bool,
}

/// Read the database's structure. Reads no user rows.
///
/// # Errors
///
/// Any `SQLite` error.
pub fn introspect(db: &Database) -> rusqlite::Result<Schema> {
    let conn = db.conn();
    let schema_version = db.schema_version()?;

    let master: Vec<(String, String, String)> = conn
        .prepare("SELECT type, name, tbl_name FROM sqlite_schema ORDER BY rowid")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let triggered: Vec<&str> = master
        .iter()
        .filter(|(t, _, _)| t == "trigger")
        .map(|(_, _, tbl)| tbl.as_str())
        .collect();
    let order: HashMap<&str, usize> = master
        .iter()
        .enumerate()
        .rev()
        .map(|(i, (_, n, _))| (n.as_str(), i))
        .collect();

    let mut entries: Vec<ListEntry> = conn
        .prepare("SELECT schema, name, type, wr, strict FROM pragma_table_list")?
        .query_map([], |r| {
            Ok(ListEntry {
                schema: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                without_rowid: r.get::<_, i64>(3)? != 0,
                strict: r.get::<_, i64>(4)? != 0,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    entries.sort_by_key(|e| order.get(e.name.as_str()).copied().unwrap_or(usize::MAX));

    let mut tables = Vec::new();
    let mut skipped = Vec::new();
    for e in entries {
        let reason = if e.schema != "main" || e.name.starts_with("sqlite_") {
            Some(SkipReason::Internal)
        } else {
            match e.kind.as_str() {
                "table" => None,
                "view" => Some(SkipReason::View),
                "virtual" => Some(SkipReason::Virtual),
                "shadow" => Some(SkipReason::Shadow),
                _ => Some(SkipReason::Internal),
            }
        };
        if let Some(reason) = reason {
            skipped.push(Skipped {
                name: e.name,
                reason,
            });
            continue;
        }
        let has_triggers = triggered.iter().any(|t| t.eq_ignore_ascii_case(&e.name));
        match read_table(conn, &e, has_triggers)? {
            Some(t) => tables.push(t),
            None => skipped.push(Skipped {
                name: e.name,
                reason: SkipReason::NoIdentity,
            }),
        }
    }

    let mut schema = Schema {
        schema_version,
        tables,
        skipped,
    };
    resolve_targets(&mut schema);
    Ok(schema)
}

fn index(v: i64) -> Option<usize> {
    usize::try_from(v).ok()
}

fn read_table(
    conn: &Connection,
    e: &ListEntry,
    has_triggers: bool,
) -> rusqlite::Result<Option<Table>> {
    let mut columns: Vec<Column> = Vec::new();
    let mut pk: Vec<(u32, usize)> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT cid, name, type, \"notnull\", dflt_value, pk, hidden \
             FROM pragma_table_xinfo(?1, 'main') ORDER BY cid",
        )?;
        let rows = stmt.query_map([&e.name], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)? != 0,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
            ))
        })?;
        for row in rows {
            let (cid, name, ty, not_null, default_sql, pk_pos, hidden) = row?;
            // Columns are addressed by position, which must equal `cid`.
            // Hidden (virtual-table) columns would break that, but virtual
            // tables are skipped before we get here.
            debug_assert_eq!(usize::try_from(cid).ok(), Some(columns.len()));
            let declared_type = ty.unwrap_or_default();
            let pk_position = u32::try_from(pk_pos).ok().filter(|p| *p > 0);
            if let Some(p) = pk_position {
                pk.push((p, columns.len()));
            }
            columns.push(Column {
                name: Ident::new(name),
                affinity: Affinity::from_declared_type(&declared_type),
                declared_type,
                not_null,
                default_sql,
                generated: hidden == 2 || hidden == 3,
                pk_position,
                is_rowid_alias: false,
            });
        }
    }
    pk.sort_unstable();
    let primary_key: Vec<usize> = pk.into_iter().map(|(_, i)| i).collect();

    let identity = if e.without_rowid {
        Identity::PrimaryKey {
            columns: primary_key.clone(),
        }
    } else {
        // `INTEGER PRIMARY KEY DESC` is not an alias: SQLite gives it an
        // automatic index with origin 'pk'.
        let pk_index: bool = conn
            .prepare("SELECT 1 FROM pragma_index_list(?1, 'main') WHERE origin = 'pk'")?
            .exists([&e.name])?;
        let alias_column = match primary_key.as_slice() {
            [i] if !pk_index
                && columns[*i]
                    .declared_type
                    .trim()
                    .eq_ignore_ascii_case("INTEGER") =>
            {
                Some(*i)
            }
            _ => None,
        };
        let accessor = if let Some(i) = alias_column {
            columns[i].name.as_str().to_owned()
        } else {
            let free = ROWID_NAMES.iter().find(|n| {
                !columns
                    .iter()
                    .any(|c| c.name.as_str().eq_ignore_ascii_case(n))
            });
            let Some(n) = free else { return Ok(None) };
            (*n).to_owned()
        };
        if let Some(i) = alias_column {
            columns[i].is_rowid_alias = true;
        }
        Identity::Rowid {
            accessor: Ident::new(accessor),
            alias_column,
        }
    };

    let foreign_keys = read_foreign_keys(conn, &e.name, &columns)?;
    let unique_keys = read_unique_keys(conn, &e.name, &primary_key)?;

    Ok(Some(Table {
        name: Ident::new(e.name.clone()),
        without_rowid: e.without_rowid,
        strict: e.strict,
        columns,
        primary_key,
        identity,
        foreign_keys,
        unique_keys,
        has_triggers,
    }))
}

fn read_unique_keys(
    conn: &Connection,
    table: &str,
    primary_key: &[usize],
) -> rusqlite::Result<Vec<Vec<usize>>> {
    let mut keys: Vec<Vec<usize>> = Vec::new();
    if !primary_key.is_empty() {
        let mut pk = primary_key.to_vec();
        pk.sort_unstable();
        keys.push(pk);
    }
    let names: Vec<String> = conn
        .prepare(
            "SELECT name FROM pragma_index_list(?1, 'main') \
             WHERE \"unique\" = 1 AND partial = 0 ORDER BY seq",
        )?
        .query_map([table], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for name in names {
        let cids: Vec<i64> = conn
            .prepare("SELECT cid FROM pragma_index_info(?1, 'main') ORDER BY seqno")?
            .query_map([&name], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let key: Option<Vec<usize>> = cids.into_iter().map(index).collect();
        let key = key.map(|mut k| {
            k.sort_unstable();
            k
        });
        if let Some(key) = key.filter(|k| !k.is_empty()) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    Ok(keys)
}

fn read_foreign_keys(
    conn: &Connection,
    table: &str,
    columns: &[Column],
) -> rusqlite::Result<Vec<ForeignKey>> {
    type Row = (i64, String, String, Option<String>, String, String);
    let rows: Vec<Row> = conn
        .prepare(
            "SELECT id, \"table\", \"from\", \"to\", on_update, on_delete \
             FROM pragma_foreign_key_list(?1, 'main') ORDER BY id, seq",
        )?
        .query_map([table], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut fks: Vec<ForeignKey> = Vec::new();
    for (id, to_table, from, to, on_update, on_delete) in rows {
        let from_idx = columns
            .iter()
            .position(|c| c.name.as_str() == from)
            .or_else(|| {
                columns
                    .iter()
                    .position(|c| c.name.as_str().eq_ignore_ascii_case(&from))
            });
        if fks.last().is_none_or(|f| f.id != id) {
            fks.push(ForeignKey {
                id,
                from: Vec::new(),
                to_table,
                to_columns: Vec::new(),
                on_delete: FkAction::parse(&on_delete),
                on_update: FkAction::parse(&on_update),
                target: None,
            });
        }
        let fk = fks.last_mut().expect("just pushed");
        if let Some(i) = from_idx {
            fk.from.push(i);
        } else {
            // SQLite guarantees the FK's `from` column exists, so a miss
            // means introspection is wrong, not the database. Skip it in
            // release builds, fail loudly in debug builds.
            debug_assert!(false, "FK column {from:?} not found in table {table:?}");
        }
        if let Some(to) = to {
            fk.to_columns.push(to);
        }
    }
    // Implicit targets (`REFERENCES parent`) have empty `to_columns`;
    // resolve_targets fills them in once the parent is known.
    Ok(fks)
}

/// Fill in implicit `to_columns` and every `target`.
fn resolve_targets(schema: &mut Schema) {
    let mut resolved: Vec<(usize, usize, Vec<String>, Option<FkTarget>)> = Vec::new();
    for (ti, t) in schema.tables.iter().enumerate() {
        for (fi, fk) in t.foreign_keys.iter().enumerate() {
            let parent = schema.table_index(&fk.to_table);
            let mut to_columns = fk.to_columns.clone();
            if to_columns.is_empty() {
                if let Some(p) = parent {
                    let pt = &schema.tables[p];
                    to_columns = pt
                        .primary_key
                        .iter()
                        .map(|&i| pt.columns[i].name.as_str().to_owned())
                        .collect();
                }
            }
            let target = parent.and_then(|p| {
                let pt = &schema.tables[p];
                let cols: Option<Vec<usize>> = to_columns.iter().map(|c| pt.column(c)).collect();
                cols.filter(|c| !c.is_empty() && c.len() == fk.from.len())
                    .map(|columns| {
                        let mut set = columns.clone();
                        set.sort_unstable();
                        let is_key = pt.unique_keys.contains(&set);
                        FkTarget {
                            table: p,
                            columns,
                            is_key,
                        }
                    })
            });
            resolved.push((ti, fi, to_columns, target));
        }
    }
    for (ti, fi, to_columns, target) in resolved {
        let fk = &mut schema.tables[ti].foreign_keys[fi];
        fk.to_columns = to_columns;
        fk.target = target;
    }
}
