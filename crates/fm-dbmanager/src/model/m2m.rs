//! Many-to-many: detecting junctions, and resolving each relation's mode,
//! headings and link fields.

use std::borrow::Cow;

use super::resolve::unmatched;
use super::{diag, ResolvedField};
use crate::config::{
    Config, ConfigDiagnostic, Defaults, DiagnosticSeverity, FieldMode, Located, M2mConfig,
    M2mEntry, RelationMode,
};
use crate::schema::{Schema, Table};

/// One side of a junction: the column, the column it references, and the
/// table that holds it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Side {
    pub column: usize,
    pub key: usize,
    pub table: usize,
}

/// A table used as many-to-many.
pub(super) struct Junction<'a> {
    pub a: Side,
    pub b: Side,
    /// Borrowed from the config, or merged from several entries for the table.
    pub entry: Option<Cow<'a, M2mEntry>>,
    /// The junction columns that are not relation columns or a rowid alias.
    pub extra: Vec<usize>,
}

fn single_fk(t: &Table, column: usize) -> Option<(usize, usize)> {
    t.foreign_keys.iter().find_map(|f| {
        let tg = f.target.as_ref()?;
        (f.from.as_slice() == [column] && tg.is_key).then(|| (tg.table, tg.columns[0]))
    })
}

fn pair(t: &Table, ca: usize, cb: usize) -> Result<(Side, Side), String> {
    let (Some((ta, ka)), Some((tb, kb))) = (single_fk(t, ca), single_fk(t, cb)) else {
        return Err(
            "both columns must be single-column foreign keys to existing tables, referencing a key"
                .into(),
        );
    };
    if ta == tb {
        return Err(
            "a self-referential junction (both keys to the same table) is not supported".into(),
        );
    }
    let mut set = vec![ca, cb];
    set.sort_unstable();
    if !t.unique_keys.contains(&set) {
        return Err("the two columns are not the primary key or a unique index".into());
    }
    Ok((
        Side {
            column: ca,
            key: ka,
            table: ta,
        },
        Side {
            column: cb,
            key: kb,
            table: tb,
        },
    ))
}

/// The two columns of the table's only two foreign keys, when they are two
/// single-column keys.
fn only_two(t: &Table) -> Option<(usize, usize)> {
    match t.foreign_keys.as_slice() {
        [x, y] if x.from.len() == 1 && y.from.len() == 1 && x.from[0] != y.from[0] => {
            Some((x.from[0], y.from[0]))
        }
        _ => None,
    }
}

fn extras(t: &Table, a: usize, b: usize) -> Vec<usize> {
    (0..t.columns.len())
        .filter(|&c| c != a && c != b && !t.columns[c].is_rowid_alias)
        .collect()
}

fn declared(t: &Table, m: &M2mConfig) -> Result<(Side, Side), String> {
    if let Some(cols) = &m.columns {
        let find = |name: &String| {
            t.column(name)
                .ok_or_else(|| format!("`columns` names `{name}`, which the table does not have"))
        };
        pair(t, find(&cols.value[0])?, find(&cols.value[1])?)
    } else {
        let (a, b) = only_two(t)
            .ok_or_else(|| "the table does not have exactly two single-column foreign keys; name them with `columns`".to_string())?;
        pair(t, a, b)
    }
}

fn detected(t: &Table) -> Option<(Side, Side)> {
    let (a, b) = only_two(t)?;
    // Besides the two relation columns, only an optional rowid alias.
    if (0..t.columns.len()).any(|c| c != a && c != b && !t.columns[c].is_rowid_alias) {
        return None;
    }
    pair(t, a, b).ok()
}

fn entry_name(e: &M2mEntry) -> (&str, crate::config::At) {
    match e {
        M2mEntry::Valid(m) => (&m.name, m.at),
        M2mEntry::Rejected { name, at } => (name, *at),
    }
}

fn valid_entry(e: &M2mEntry) -> Option<&M2mConfig> {
    match e {
        M2mEntry::Valid(m) => Some(m),
        M2mEntry::Rejected { .. } => None,
    }
}

/// Several entries for one junction. Each one after the first gets a
/// warning. The relation is hidden if any entry asks for it. Otherwise it
/// stays a relation (so a declared junction is not turned into an editable
/// table) but read-only, taken from the first valid entry that declares
/// `columns`, or else the first valid entry; with no valid entry the merge is
/// a rejected entry.
fn merge_entries(table: &str, group: &[&M2mEntry], diags: &mut Vec<ConfigDiagnostic>) -> M2mEntry {
    for e in &group[1..] {
        diags.push(diag(
            DiagnosticSeverity::Warning,
            format!(
                "many-to-many `{table}` is configured more than once; the relation is hidden if any entry hides it, and read-only otherwise"
            ),
            Some(entry_name(e).1),
            Some(table),
            None,
        ));
    }
    let (name, at) = entry_name(group[0]);
    let hidden = group.iter().filter_map(|e| valid_entry(e)).any(|m| {
        m.mode
            .as_ref()
            .is_some_and(|l| l.value == RelationMode::Hidden)
    });
    // Keep a declaration of the columns, if there is one, so the relation is
    // still found.
    let base = group
        .iter()
        .filter_map(|e| valid_entry(e))
        .find(|m| m.columns.is_some())
        .or_else(|| group.iter().find_map(|e| valid_entry(e)));
    let Some(base) = base else {
        return M2mEntry::Rejected {
            name: name.to_owned(),
            at,
        };
    };
    let mut config = base.clone();
    // Disabling one of the entries must not turn the junction into an
    // ordinary, editable table.
    config.enabled = None;
    config.mode = Some(Located {
        value: if hidden {
            RelationMode::Hidden
        } else {
            RelationMode::ReadOnly
        },
        at: config.at,
    });
    M2mEntry::Valid(config)
}

/// Finds the junctions. Parallel to `schema.tables`.
pub(super) fn detect<'a>(
    schema: &Schema,
    config: &'a Config,
    diags: &mut Vec<ConfigDiagnostic>,
) -> (Vec<Option<Junction<'a>>>, Vec<bool>) {
    let mut groups: Vec<Vec<&M2mEntry>> = vec![Vec::new(); schema.tables.len()];
    for e in &config.many_to_many {
        let (name, at) = entry_name(e);
        match schema.table_index(name) {
            Some(i) => groups[i].push(e),
            None => diags.push(unmatched(schema, "junction", name, at)),
        }
    }
    let entries: Vec<Option<Cow<'a, M2mEntry>>> = groups
        .iter()
        .enumerate()
        .map(|(i, g)| match g.as_slice() {
            [] => None,
            [one] => Some(Cow::Borrowed(*one)),
            many => Some(Cow::Owned(merge_entries(
                schema.tables[i].name.as_str(),
                many,
                diags,
            ))),
        })
        .collect();
    // A rejected entry that was needed to declare the junction leaves an
    // ordinary table, which is forced read-only (`forced`).
    let mut forced = vec![false; schema.tables.len()];
    let mut out = Vec::with_capacity(schema.tables.len());
    for (i, (t, entry)) in schema.tables.iter().zip(entries).enumerate() {
        let sides = match entry.as_deref() {
            Some(M2mEntry::Valid(m)) if m.enabled.as_ref().is_some_and(|e| !e.value) => None,
            Some(M2mEntry::Valid(m)) => match declared(t, m) {
                Ok(s) => Some(s),
                Err(why) => {
                    diags.push(diag(
                        DiagnosticSeverity::Warning,
                        format!("many-to-many `{}` is not used: {why}", t.name),
                        Some(m.at),
                        Some(t.name.as_str()),
                        None,
                    ));
                    None
                }
            },
            Some(M2mEntry::Rejected { .. }) => {
                let s = detected(t);
                forced[i] = s.is_none();
                s
            }
            None => detected(t),
        };
        out.push(sides.map(|(a, b)| Junction {
            extra: extras(t, a.column, b.column),
            a,
            b,
            entry,
        }));
    }
    (out, forced)
}

/// A relation with its mode, headings and link fields resolved.
pub(super) struct Relation {
    pub junction: usize,
    pub a: Side,
    pub b: Side,
    pub mode: RelationMode,
    pub can_add: bool,
    /// Table index to heading text.
    pub headings: Vec<(usize, String)>,
    /// Link fields in junction column order.
    pub link_fields: Vec<ResolvedField>,
}

fn relation_mode(
    entry: Option<&M2mEntry>,
    defaults: Defaults,
) -> (RelationMode, Option<crate::config::Located<RelationMode>>) {
    match entry {
        Some(M2mEntry::Valid(m)) => match &m.mode {
            Some(l) => (l.value, Some(l.clone())),
            None => (defaults.relation_mode, None),
        },
        Some(M2mEntry::Rejected { .. }) => (RelationMode::ReadOnly, None),
        None => (defaults.relation_mode, None),
    }
}

/// Builds every relation, in schema order of the junction. `fields` has every
/// column of every table resolved.
pub(super) fn build(
    schema: &Schema,
    config: &Config,
    junctions: &[Option<Junction<'_>>],
    fields: &[Vec<ResolvedField>],
    diags: &mut Vec<ConfigDiagnostic>,
) -> Vec<Relation> {
    let mut out = Vec::new();
    for (ji, j) in junctions.iter().enumerate() {
        let Some(j) = j else { continue };
        let t = &schema.tables[ji];
        let (mut mode, explicit) = relation_mode(j.entry.as_deref(), config.defaults);
        let link_fields: Vec<ResolvedField> =
            j.extra.iter().map(|&c| fields[ji][c].clone()).collect();
        let blocking = link_fields.iter().find(|f| {
            f.required_on_create && !matches!(f.mode, FieldMode::Editable | FieldMode::CreateOnly)
        });
        let can_add = blocking.is_none();
        if let (Some(f), RelationMode::Editable) = (blocking, mode) {
            let message = format!(
                "links cannot be added to `{}`: `{}` is NOT NULL without a default and no editable or create-only link field supplies it, so the relation is read-only",
                t.name, f.name
            );
            let (severity, at) = match &explicit {
                Some(l) => (DiagnosticSeverity::Error, Some(l.at)),
                None => (
                    DiagnosticSeverity::Warning,
                    match j.entry.as_deref() {
                        Some(M2mEntry::Valid(m)) => Some(m.at),
                        _ => None,
                    },
                ),
            };
            diags.push(diag(
                severity,
                message,
                at,
                Some(t.name.as_str()),
                Some(f.name.as_str()),
            ));
            mode = RelationMode::ReadOnly;
        }
        let mut headings = Vec::new();
        if let Some(M2mEntry::Valid(m)) = j.entry.as_deref() {
            for h in &m.headings {
                match schema.table_index(&h.side) {
                    Some(i) if i == j.a.table || i == j.b.table => {
                        headings.push((i, h.text.clone()));
                    }
                    _ => diags.push(diag(
                        DiagnosticSeverity::Warning,
                        format!(
                            "many-to-many `{}`: `headings` names `{}`, which is not one of its two sides",
                            t.name, h.side
                        ),
                        Some(h.at),
                        Some(t.name.as_str()),
                        None,
                    )),
                }
            }
        }
        out.push(Relation {
            junction: ji,
            a: j.a,
            b: j.b,
            mode,
            can_add,
            headings,
            link_fields,
        });
    }
    out
}
