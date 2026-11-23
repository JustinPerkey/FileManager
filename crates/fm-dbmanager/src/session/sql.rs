//! The only module that assembles SQL text. Names arrive as `&Ident` and are
//! quoted with `Ident::quoted`; every value is a bound parameter.

use rusqlite::types::Value;

use crate::schema::Ident;

/// The listed table's alias; a fixed literal, never a database name.
const T: &str = "t";
/// The junction's alias.
const J: &str = "j";

/// SQL text and its parameters, in order of appearance.
pub(super) struct Query {
    pub sql: String,
    pub params: Vec<Value>,
}

/// "A junction row joins this row to that one": the junction row's `this`
/// column equals `this_value` and its `other` column equals the listed
/// table's `other_key` column.
pub(super) struct Link<'a> {
    pub junction: &'a Ident,
    pub junction_this: &'a Ident,
    pub junction_other: &'a Ident,
    pub other_key: &'a Ident,
    pub this_value: Value,
}

pub(super) enum Filter<'a> {
    /// `column = value`.
    Eq(&'a Ident, Value),
    /// Any column contains the text, `LIKE` with `%`, `_` and `\` escaped.
    Search(Vec<&'a Ident>, String),
    /// Only rows joined by a junction row.
    Linked(Link<'a>),
}

pub(super) struct Select<'a> {
    pub table: &'a Ident,
    pub columns: Vec<&'a Ident>,
    /// When set, rows add a trailing 0/1 column: is the row linked.
    pub linked_flag: Option<Link<'a>>,
    pub filters: Vec<Filter<'a>>,
    /// Column and `descending`.
    pub order: Vec<(&'a Ident, bool)>,
    pub page: Option<(u32, u32)>,
}

fn escape_like(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('%');
    for c in text.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

fn column(alias: &str, name: &Ident) -> String {
    format!("{alias}.{}", name.quoted())
}

fn exists(link: Link<'_>, params: &mut Vec<Value>) -> String {
    params.push(link.this_value);
    format!(
        "EXISTS (SELECT 1 FROM {} AS {J} WHERE {} = ? AND {} = {})",
        link.junction.quoted(),
        column(J, link.junction_this),
        column(J, link.junction_other),
        column(T, link.other_key),
    )
}

impl<'a> Select<'a> {
    pub(super) fn new(table: &'a Ident) -> Self {
        Select {
            table,
            columns: Vec::new(),
            linked_flag: None,
            filters: Vec::new(),
            order: Vec::new(),
            page: None,
        }
    }

    /// `FROM ... WHERE ...` and its parameters.
    fn tail(filters: Vec<Filter<'_>>, table: &Ident, params: &mut Vec<Value>) -> String {
        let mut out = format!(" FROM {} AS {T}", table.quoted());
        let mut conds = Vec::new();
        for f in filters {
            match f {
                Filter::Eq(c, v) => {
                    params.push(v);
                    conds.push(format!("{} = ?", column(T, c)));
                }
                Filter::Search(cols, text) => {
                    let pattern = escape_like(&text);
                    let any: Vec<String> = cols
                        .iter()
                        .map(|c| {
                            params.push(Value::Text(pattern.clone()));
                            format!("{} LIKE ? ESCAPE '\\'", column(T, c))
                        })
                        .collect();
                    if any.is_empty() {
                        conds.push("0".to_owned());
                    } else {
                        conds.push(format!("({})", any.join(" OR ")));
                    }
                }
                Filter::Linked(link) => conds.push(exists(link, params)),
            }
        }
        if !conds.is_empty() {
            out.push_str(" WHERE ");
            out.push_str(&conds.join(" AND "));
        }
        out
    }

    pub(super) fn rows(self) -> Query {
        let mut params = Vec::new();
        let mut items: Vec<String> = self.columns.iter().map(|c| column(T, c)).collect();
        if let Some(link) = self.linked_flag {
            items.push(exists(link, &mut params));
        }
        let mut sql = format!("SELECT {}", items.join(", "));
        sql.push_str(&Self::tail(self.filters, self.table, &mut params));
        if !self.order.is_empty() {
            let terms: Vec<String> = self
                .order
                .iter()
                .map(|(c, desc)| format!("{} {}", column(T, c), if *desc { "DESC" } else { "ASC" }))
                .collect();
            sql.push_str(" ORDER BY ");
            sql.push_str(&terms.join(", "));
        }
        if let Some((limit, offset)) = self.page {
            sql.push_str(" LIMIT ? OFFSET ?");
            params.push(Value::Integer(i64::from(limit)));
            params.push(Value::Integer(i64::from(offset)));
        }
        Query { sql, params }
    }

    pub(super) fn count(self) -> Query {
        let mut params = Vec::new();
        let mut sql = "SELECT count(*)".to_owned();
        sql.push_str(&Self::tail(self.filters, self.table, &mut params));
        Query { sql, params }
    }
}

/// `UPDATE <table> SET <col> = ?, ... WHERE <col> = ? AND ...`; the set
/// values come first in the parameters.
pub(super) fn update(
    table: &Ident,
    sets: Vec<(&Ident, Value)>,
    wheres: Vec<(&Ident, Value)>,
) -> Query {
    let mut params = Vec::new();
    let mut assigns = Vec::new();
    for (c, v) in sets {
        assigns.push(format!("{} = ?", c.quoted()));
        params.push(v);
    }
    let mut conds = Vec::new();
    for (c, v) in wheres {
        conds.push(format!("{} = ?", c.quoted()));
        params.push(v);
    }
    Query {
        sql: format!(
            "UPDATE {} SET {} WHERE {}",
            table.quoted(),
            assigns.join(", "),
            conds.join(" AND ")
        ),
        params,
    }
}

/// `INSERT INTO <table> (<cols>) VALUES (?, ...)`, or `DEFAULT VALUES`.
pub(super) fn insert(table: &Ident, values: Vec<(&Ident, Value)>) -> Query {
    if values.is_empty() {
        return Query {
            sql: format!("INSERT INTO {} DEFAULT VALUES", table.quoted()),
            params: Vec::new(),
        };
    }
    let cols: Vec<String> = values.iter().map(|(c, _)| c.quoted()).collect();
    let marks = vec!["?"; values.len()].join(", ");
    Query {
        sql: format!(
            "INSERT INTO {} ({}) VALUES ({marks})",
            table.quoted(),
            cols.join(", ")
        ),
        params: values.into_iter().map(|(_, v)| v).collect(),
    }
}

/// `PRAGMA schema_version`: changes whenever the schema does.
pub(super) fn schema_version(conn: &rusqlite::Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA schema_version", [], |r| r.get(0))
}

/// `PRAGMA data_version`: changes, on this connection, whenever another
/// connection commits to the database.
pub(super) fn data_version(conn: &rusqlite::Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA data_version", [], |r| r.get(0))
}
