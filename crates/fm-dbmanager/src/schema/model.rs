//! The schema model produced by introspection.

use super::ident::Ident;

/// A database's structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub schema_version: i64,
    /// Object tables, in `sqlite_schema` order.
    pub tables: Vec<Table>,
    pub skipped: Vec<Skipped>,
}

/// Something that is not an object table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub name: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    View,
    Virtual,
    Shadow,
    Internal,
    /// A rowid table with no way to address a row.
    NoIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub name: Ident,
    pub without_rowid: bool,
    pub strict: bool,
    pub columns: Vec<Column>,
    /// Column indexes ordered by primary-key position.
    pub primary_key: Vec<usize>,
    pub identity: Identity,
    pub foreign_keys: Vec<ForeignKey>,
    /// Column indexes of each unique key; the primary key counts as one.
    /// Keys are sets: each is sorted by column index, its order is not
    /// meaningful, and equal sets appear once.
    pub unique_keys: Vec<Vec<usize>>,
    pub has_triggers: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Affinity {
    Integer,
    Text,
    Blob,
    Real,
    Numeric,
}

impl Affinity {
    /// `SQLite`'s affinity rules on a declared type.
    #[must_use]
    pub fn from_declared_type(declared: &str) -> Affinity {
        let t = declared.to_ascii_uppercase();
        if t.contains("INT") {
            Affinity::Integer
        } else if t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT") {
            Affinity::Text
        } else if t.contains("BLOB") || t.is_empty() {
            Affinity::Blob
        } else if t.contains("REAL") || t.contains("FLOA") || t.contains("DOUB") {
            Affinity::Real
        } else {
            Affinity::Numeric
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: Ident,
    /// As written; may be empty.
    pub declared_type: String,
    pub affinity: Affinity,
    pub not_null: bool,
    pub default_sql: Option<String>,
    pub generated: bool,
    pub pk_position: Option<u32>,
    pub is_rowid_alias: bool,
}

impl Column {
    /// The declared type is exactly `INTEGER` (surrounding spaces ignored).
    /// Introspection uses this for rowid-alias detection, and the save rules
    /// for the `INTEGER PRIMARY KEY DESC` exemption, so the two cannot differ.
    #[must_use]
    pub(crate) fn declared_integer(&self) -> bool {
        self.declared_type.trim().eq_ignore_ascii_case("INTEGER")
    }
}

impl Table {
    /// The columns a create must supply beyond `required_on_create`, and
    /// whose NULL is refused:
    ///
    /// - the whole primary key of a `WITHOUT ROWID` table, even when a column
    ///   has a default (the new row's key cannot be read back after the
    ///   insert);
    /// - the declared primary key of a rowid table that is not a rowid alias
    ///   and has no default.
    ///
    /// **Exemption:** a single-column primary key declared exactly
    /// `INTEGER PRIMARY KEY DESC` is not a rowid alias (a SQLite quirk) but is
    /// not required either. The table's objects are keyed by the rowid, so
    /// the new key is read back by `last_insert_rowid()`, and the column
    /// stays an ordinary nullable one.
    #[must_use]
    pub fn required_key_columns(&self) -> Vec<usize> {
        match &self.identity {
            Identity::PrimaryKey { columns } => columns.clone(),
            Identity::Rowid {
                alias_column: None, ..
            } => self
                .primary_key
                .iter()
                .copied()
                .filter(|&c| {
                    let col = &self.columns[c];
                    let desc_integer = self.primary_key.len() == 1 && col.declared_integer();
                    col.default_sql.is_none() && !desc_integer
                })
                .collect(),
            Identity::Rowid { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identity {
    Rowid {
        accessor: Ident,
        alias_column: Option<usize>,
    },
    PrimaryKey {
        columns: Vec<usize>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FkAction {
    NoAction,
    Restrict,
    SetNull,
    SetDefault,
    Cascade,
}

impl FkAction {
    pub(crate) fn parse(text: &str) -> FkAction {
        match text.to_ascii_uppercase().as_str() {
            "RESTRICT" => FkAction::Restrict,
            "SET NULL" => FkAction::SetNull,
            "SET DEFAULT" => FkAction::SetDefault,
            "CASCADE" => FkAction::Cascade,
            _ => FkAction::NoAction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FkTarget {
    /// Index into `Schema::tables`.
    pub table: usize,
    pub columns: Vec<usize>,
    /// The referenced columns, as a set, equal the parent's primary key or
    /// one of its unique keys. When false, `SQLite` raises "foreign key
    /// mismatch" on any write that touches the FK.
    pub is_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKey {
    pub id: i64,
    pub from: Vec<usize>,
    /// As written; may name a table that does not exist.
    pub to_table: String,
    pub to_columns: Vec<String>,
    pub on_delete: FkAction,
    pub on_update: FkAction,
    pub target: Option<FkTarget>,
}

/// Exact match first, then ASCII case-insensitive, as `SQLite` resolves names.
fn find_name<'a>(mut names: impl Iterator<Item = &'a str> + Clone, wanted: &str) -> Option<usize> {
    names
        .clone()
        .position(|n| n == wanted)
        .or_else(|| names.position(|n| n.eq_ignore_ascii_case(wanted)))
}

impl Table {
    /// Index of the column called `name`.
    #[must_use]
    pub fn column(&self, name: &str) -> Option<usize> {
        find_name(self.columns.iter().map(|c| c.name.as_str()), name)
    }
}

impl Schema {
    /// The table called `name`.
    #[must_use]
    pub fn table(&self, name: &str) -> Option<&Table> {
        self.table_index(name).map(|i| &self.tables[i])
    }

    pub(crate) fn table_index(&self, name: &str) -> Option<usize> {
        find_name(self.tables.iter().map(|t| t.name.as_str()), name)
    }

    /// Every foreign key, in any table, whose target is `table`, with the
    /// index of the table that holds it.
    pub fn referencing(&self, table: usize) -> impl Iterator<Item = (usize, &ForeignKey)> {
        self.tables.iter().enumerate().flat_map(move |(i, t)| {
            t.foreign_keys
                .iter()
                .filter(move |fk| fk.target.as_ref().is_some_and(|tg| tg.table == table))
                .map(move |fk| (i, fk))
        })
    }
}
