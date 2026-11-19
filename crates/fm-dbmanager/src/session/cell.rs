//! A stored value, owned, exactly as `SQLite` holds it (text as bytes, so the
//! version hash does not depend on lossy decoding).

use rusqlite::types::{Value, ValueRef};

use crate::value::DbValue;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Cell {
    Null,
    Integer(i64),
    Real(f64),
    Text(Vec<u8>),
    Blob(Vec<u8>),
}

impl From<ValueRef<'_>> for Cell {
    fn from(v: ValueRef<'_>) -> Self {
        match v {
            ValueRef::Null => Cell::Null,
            ValueRef::Integer(n) => Cell::Integer(n),
            ValueRef::Real(x) => Cell::Real(x),
            ValueRef::Text(t) => Cell::Text(t.to_vec()),
            ValueRef::Blob(b) => Cell::Blob(b.to_vec()),
        }
    }
}

impl Cell {
    pub(super) fn is_null(&self) -> bool {
        matches!(self, Cell::Null)
    }

    /// The value as stored, as the boundary shows it.
    pub(super) fn db_value(&self) -> DbValue {
        match self {
            Cell::Null => DbValue::Null,
            Cell::Integer(n) => DbValue::Integer(n.to_string()),
            Cell::Real(x) => DbValue::Real(x.to_string()),
            Cell::Text(t) => DbValue::Text(String::from_utf8_lossy(t).into_owned()),
            Cell::Blob(b) => DbValue::Blob {
                size: b.len() as u64,
            },
        }
    }

    /// The parameter that finds this value again.
    pub(super) fn param(&self) -> Value {
        match self {
            Cell::Null => Value::Null,
            Cell::Integer(n) => Value::Integer(*n),
            Cell::Real(x) => Value::Real(*x),
            Cell::Text(t) => Value::Text(String::from_utf8_lossy(t).into_owned()),
            Cell::Blob(b) => Value::Blob(b.clone()),
        }
    }
}

static NULL: Cell = Cell::Null;

/// Some columns of one row, by column index of its table, plus the rowid when
/// the table is addressed by it.
#[derive(Debug)]
pub(super) struct Row {
    cells: Vec<Option<Cell>>,
    pub rowid: Option<i64>,
}

impl Row {
    /// Builds a row from statement output: the cells of `columns` in order,
    /// then the rowid if `with_rowid`.
    pub(super) fn new(
        column_count: usize,
        columns: &[usize],
        with_rowid: bool,
        cells: &[Cell],
    ) -> Row {
        let mut by_column = vec![None; column_count];
        for (&c, cell) in columns.iter().zip(cells) {
            by_column[c] = Some(cell.clone());
        }
        let rowid = if with_rowid {
            match cells.get(columns.len()) {
                Some(Cell::Integer(n)) => Some(*n),
                _ => None,
            }
        } else {
            None
        };
        Row {
            cells: by_column,
            rowid,
        }
    }

    /// The cell of column `c`; NULL when the column was not selected.
    pub(super) fn cell(&self, c: usize) -> &Cell {
        self.cells.get(c).and_then(Option::as_ref).unwrap_or(&NULL)
    }
}
