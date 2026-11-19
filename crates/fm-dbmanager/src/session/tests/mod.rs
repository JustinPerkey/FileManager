//! Session tests. Every test builds its database in its own temp dir.

mod config;
mod lists;
mod objects;
mod relations;

use std::path::PathBuf;

use super::*;
use crate::config::{ConfigStatus, LoadedConfig, TableMode};
use crate::value::DbValue;

pub(super) const SCHEMA: &str = include_str!("../../../../../examples/dbmanager/schema.sql");
pub(super) const EXAMPLE: &str = include_str!("../../../../../examples/dbmanager/example.toml");

pub(super) struct Fixture {
    _dir: tempfile::TempDir,
    pub db: PathBuf,
    pub cfg: PathBuf,
}

impl Fixture {
    pub fn new(sql: &str, toml: Option<&str>) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("t.db");
        rusqlite::Connection::open(&db)
            .unwrap()
            .execute_batch(sql)
            .unwrap();
        let cfg = dir.path().join("config.toml");
        if let Some(t) = toml {
            std::fs::write(&cfg, t).unwrap();
        }
        Fixture { _dir: dir, db, cfg }
    }

    pub fn example() -> Fixture {
        Fixture::new(SCHEMA, Some(EXAMPLE))
    }

    pub fn session(&self) -> Session {
        Session::open(&self.db, LoadedConfig::load(&self.cfg)).unwrap()
    }

    /// Runs SQL on a second connection with foreign keys off.
    pub fn exec(&self, sql: &str) {
        let c = rusqlite::Connection::open(&self.db).unwrap();
        c.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        c.execute_batch(sql).unwrap();
    }
}

pub(super) fn rk(n: i64) -> ObjectKey {
    ObjectKey::Rowid {
        value: n.to_string(),
    }
}

pub(super) fn pk(text: &str) -> ObjectKey {
    ObjectKey::Primary {
        values: vec![DbValue::Text(text.into())],
    }
}

pub(super) fn lq(search: Option<&str>, offset: u32, limit: u32) -> ListQuery {
    ListQuery {
        search: search.map(str::to_owned),
        offset,
        limit,
        referencing: None,
    }
}

pub(super) fn mq(search: Option<&str>, offset: u32, limit: u32) -> M2mQuery {
    M2mQuery {
        search: search.map(str::to_owned),
        offset,
        limit,
        linked_only: false,
        only: None,
    }
}

pub(super) fn labels(p: &ObjectPage) -> Vec<&str> {
    p.items.iter().map(|i| i.label.as_str()).collect()
}

pub(super) fn field<'a>(o: &'a ObjectView, column: &str) -> &'a FieldValue {
    o.fields
        .iter()
        .find(|f| f.column == column)
        .unwrap_or_else(|| panic!("no field {column}"))
}

pub(super) fn mode_of(s: &Session, table: &str) -> TableMode {
    s.table_views()
        .iter()
        .find(|t| t.name == table)
        .unwrap_or_else(|| panic!("no table {table}"))
        .mode
}

pub(super) fn relation_to(s: &Session, table: &str, from: &str) -> String {
    s.table_views()
        .iter()
        .find(|t| t.name == table)
        .unwrap()
        .referenced_by
        .iter()
        .find(|r| r.table == from)
        .unwrap()
        .relation
        .clone()
}

pub(super) fn status(f: &Fixture) -> ConfigStatus {
    LoadedConfig::load(&f.cfg).status()
}
