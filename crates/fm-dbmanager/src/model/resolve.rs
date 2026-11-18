//! The driver: matches config entries to tables, then resolves modes, fields,
//! relations and tables in dependency order.

use super::fields::{resolve_field, Env, Source, TableCtx};
use super::labels::{display_order, resolve_label, resolve_order, search_columns};
use super::m2m::{build, detect, Junction, Relation};
use super::{diag, M2mSide, Model, ResolvedField, ResolvedTable, ReverseRelation};
use crate::config::{
    At, ConfigDiagnostic, ConfigReport, DiagnosticSeverity, FieldEntry, FieldMode, Located,
    RelationMode, TableConfig, TableEntry, TableMode,
};
use crate::schema::{Schema, SkipReason};

/// What a table's mode comes to, once junctions are known.
pub(super) struct TableInfo {
    pub mode: TableMode,
    /// Shown as a table of its own (or a pickable one). False for a junction
    /// used as many-to-many unless its entry lists it, and for hidden tables.
    pub listed: bool,
    pub read_only_fields: bool,
    pub warn_read_only: bool,
}

/// A warning for a config entry that names nothing usable in the database.
pub(super) fn unmatched(schema: &Schema, what: &str, name: &str, at: At) -> ConfigDiagnostic {
    let message = match schema
        .skipped
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
    {
        Some(s) => {
            let kind = match s.reason {
                SkipReason::View => "a view",
                SkipReason::Virtual => "a virtual table",
                SkipReason::Shadow => "a shadow table",
                SkipReason::Internal => "an internal table",
                SkipReason::NoIdentity => "a table with no usable identity",
            };
            format!("{what} `{name}` is {kind}, which is not managed; the entry is ignored")
        }
        None => format!("unknown {what} `{name}`; the entry is ignored"),
    };
    diag(
        DiagnosticSeverity::Warning,
        message,
        Some(at),
        Some(name),
        None,
    )
}

fn entry_name(e: &TableEntry) -> (&str, At) {
    match e {
        TableEntry::Valid(t) => (&t.name, t.at),
        TableEntry::Rejected { name, at, .. } => (name, *at),
    }
}

fn valid(e: Option<&TableEntry>) -> Option<&TableConfig> {
    match e {
        Some(TableEntry::Valid(t)) => Some(t),
        _ => None,
    }
}

fn table_info(
    entry: Option<&TableEntry>,
    junction: bool,
    default_mode: TableMode,
    diags: &mut Vec<ConfigDiagnostic>,
    table: &str,
) -> TableInfo {
    let explicit: Option<&Located<TableMode>> = valid(entry).and_then(|t| t.mode.as_ref());
    let hidden_rejected = matches!(entry, Some(TableEntry::Rejected { hidden: true, .. }));
    if junction {
        let (mode, listed) = match (entry, explicit) {
            (Some(TableEntry::Rejected { hidden: true, .. }), _) => (TableMode::Hidden, false),
            (Some(TableEntry::Rejected { .. }) | None, _) | (_, None) => {
                (TableMode::Editable, false)
            }
            (_, Some(l)) => match l.value {
                TableMode::Hidden => (TableMode::Hidden, false),
                TableMode::Pickable => {
                    diags.push(diag(
                        DiagnosticSeverity::Warning,
                        format!(
                            "table `{table}` is used as many-to-many; `pickable` only means it is not listed"
                        ),
                        Some(l.at),
                        Some(table),
                        None,
                    ));
                    (TableMode::Editable, false)
                }
                m @ (TableMode::Editable | TableMode::ReadOnly) => (m, true),
            },
        };
        let ro = explicit.is_some_and(|l| l.value == TableMode::ReadOnly);
        return TableInfo {
            mode,
            listed,
            read_only_fields: ro,
            warn_read_only: ro,
        };
    }
    let mode = match entry {
        Some(TableEntry::Rejected { .. }) => {
            if hidden_rejected {
                TableMode::Hidden
            } else {
                TableMode::ReadOnly
            }
        }
        _ => explicit.map_or(default_mode, |l| l.value),
    };
    TableInfo {
        mode,
        listed: mode != TableMode::Hidden,
        read_only_fields: mode != TableMode::Editable,
        warn_read_only: matches!(mode, TableMode::ReadOnly | TableMode::Pickable)
            && !matches!(entry, Some(TableEntry::Rejected { .. })),
    }
}

/// One source per column of table `ti`, and the unknown-field warnings.
fn field_sources<'a>(
    schema: &Schema,
    ti: usize,
    entry: Option<&'a TableEntry>,
    diags: &mut Vec<ConfigDiagnostic>,
) -> Vec<Source<'a>> {
    let table = &schema.tables[ti];
    let n = table.columns.len();
    match entry {
        Some(TableEntry::Rejected { fields, .. }) => {
            let mut hidden = vec![false; n];
            for f in fields {
                let (name, h) = match f {
                    FieldEntry::Valid(c) => (
                        &c.name,
                        c.mode
                            .as_ref()
                            .is_some_and(|m| m.value == FieldMode::Hidden),
                    ),
                    FieldEntry::Rejected { name, hidden, .. } => (name, *hidden),
                };
                if let (Some(i), true) = (table.column(name), h) {
                    hidden[i] = true;
                }
            }
            hidden
                .into_iter()
                .map(|hidden| Source::Narrowed { hidden })
                .collect()
        }
        Some(TableEntry::Valid(t)) => {
            let mut out: Vec<Option<Source<'a>>> = (0..n).map(|_| None).collect();
            for f in &t.fields {
                let (name, at) = match f {
                    FieldEntry::Valid(c) => (&c.name, c.at),
                    FieldEntry::Rejected { name, at, .. } => (name, *at),
                };
                let Some(i) = table.column(name) else {
                    diags.push(diag(
                        DiagnosticSeverity::Warning,
                        format!(
                            "table `{}` has no column `{name}`; the entry is ignored",
                            table.name
                        ),
                        Some(at),
                        Some(table.name.as_str()),
                        Some(name),
                    ));
                    continue;
                };
                if out[i].is_none() {
                    out[i] = Some(match f {
                        FieldEntry::Valid(c) => Source::Config(Some(c)),
                        FieldEntry::Rejected { hidden, .. } => Source::Narrowed { hidden: *hidden },
                    });
                }
            }
            out.into_iter()
                .map(|s| s.unwrap_or(Source::Config(None)))
                .collect()
        }
        None => (0..n).map(|_| Source::Config(None)).collect(),
    }
}

fn foreign_key_warnings(
    schema: &Schema,
    ti: usize,
    fields: &[ResolvedField],
    diags: &mut Vec<ConfigDiagnostic>,
) {
    let table = &schema.tables[ti];
    for fk in &table.foreign_keys {
        let Some(&first) = fk.from.first() else {
            continue;
        };
        if fields[first].mode == FieldMode::Hidden {
            continue;
        }
        let column = table.columns[first].name.as_str();
        let parent = if fk.to_columns.is_empty() {
            fk.to_table.clone()
        } else {
            format!("{}({})", fk.to_table, fk.to_columns.join(", "))
        };
        let message = match &fk.target {
            None => format!(
                "the foreign key from `{}.{column}` to `{parent}` refers to a table or column that does not exist; `{column}` is a plain field",
                table.name
            ),
            Some(t) if !t.is_key => format!(
                "the foreign key from `{}.{column}` to `{parent}` does not reference a primary or unique key; SQLite will refuse changes that touch it",
                table.name
            ),
            Some(_) => continue,
        };
        diags.push(diag(
            DiagnosticSeverity::Warning,
            message,
            None,
            Some(table.name.as_str()),
            Some(column),
        ));
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one pass per stage over the same tables; the stages share many locals"
)]
/// Lays the config over the schema. Never fails: every mismatch is a
/// diagnostic, and an error never leaves anything more editable than its
/// entry would have.
#[must_use]
pub fn resolve(schema: &Schema, report: &ConfigReport) -> Model {
    let config = &report.config;
    let mut diags: Vec<ConfigDiagnostic> = report.diagnostics.clone();
    let n = schema.tables.len();

    // Config entries to tables.
    let mut entries: Vec<Option<&TableEntry>> = vec![None; n];
    for e in &config.tables {
        let (name, at) = entry_name(e);
        match schema.table_index(name) {
            Some(i) => {
                if entries[i].is_none() {
                    entries[i] = Some(e);
                } else {
                    diags.push(diag(
                        DiagnosticSeverity::Warning,
                        format!(
                            "table `{}` is already configured above; this entry is ignored",
                            schema.tables[i].name
                        ),
                        Some(at),
                        Some(schema.tables[i].name.as_str()),
                        None,
                    ));
                }
            }
            None => diags.push(unmatched(schema, "table", name, at)),
        }
    }
    let junctions: Vec<Option<Junction<'_>>> = detect(schema, config, &mut diags);

    // Modes and names.
    let infos: Vec<TableInfo> = (0..n)
        .map(|i| {
            table_info(
                entries[i],
                junctions[i].is_some(),
                config.defaults.table_mode,
                &mut diags,
                schema.tables[i].name.as_str(),
            )
        })
        .collect();
    let hidden_tables: Vec<bool> = infos.iter().map(|i| i.mode == TableMode::Hidden).collect();
    let display_names: Vec<String> = (0..n)
        .map(|i| {
            valid(entries[i])
                .and_then(|t| t.display_name.clone())
                .unwrap_or_else(|| schema.tables[i].name.as_str().to_owned())
        })
        .collect();

    // Fields: every column of every table.
    let env = Env {
        schema,
        defaults: config.defaults,
        hidden_tables: &hidden_tables,
        display_names: &display_names,
    };
    let all_fields: Vec<Vec<ResolvedField>> = (0..n)
        .map(|ti| {
            let sources = field_sources(schema, ti, entries[ti], &mut diags);
            let tctx = TableCtx {
                read_only: infos[ti].read_only_fields,
                warn_read_only: infos[ti].warn_read_only,
            };
            sources
                .iter()
                .enumerate()
                .map(|(ci, s)| resolve_field(&env, ti, ci, s, &tctx, &mut diags))
                .collect()
        })
        .collect();

    let relations: Vec<Relation> = build(schema, config, &junctions, &all_fields, &mut diags);

    // Tables.
    let mut tables: Vec<Option<ResolvedTable>> = Vec::with_capacity(n);
    for ti in 0..n {
        let table = &schema.tables[ti];
        let info = &infos[ti];
        let tcfg = valid(entries[ti]);
        let editable = info.mode == TableMode::Editable;
        if let (Some(c), false) = (tcfg, editable) {
            for (key, l) in [
                ("allow_create", &c.allow_create),
                ("allow_delete", &c.allow_delete),
            ] {
                if let Some(l) = l {
                    diags.push(diag(
                        DiagnosticSeverity::Warning,
                        format!(
                            "table `{}`: `{key}` is ignored because the table is not editable",
                            table.name
                        ),
                        Some(l.at),
                        Some(table.name.as_str()),
                        None,
                    ));
                }
            }
        }
        if !info.listed || info.mode == TableMode::Hidden {
            tables.push(None);
            continue;
        }
        let fields = &all_fields[ti];
        foreign_key_warnings(schema, ti, fields, &mut diags);

        let label = resolve_label(table, tcfg, fields, &mut diags);
        let search = search_columns(&label, fields);
        let order = resolve_order(table, tcfg, fields, &label, &mut diags);
        let display = display_order(table, tcfg, fields, &mut diags);

        let allow_create = tcfg.and_then(|c| c.allow_create.as_ref());
        let mut can_create = editable && allow_create.is_none_or(|l| l.value);
        if can_create {
            let blocking = fields.iter().find(|f| {
                f.required_on_create
                    && !matches!(f.mode, FieldMode::Editable | FieldMode::CreateOnly)
            });
            if let Some(f) = blocking {
                can_create = false;
                let (severity, at) = match allow_create {
                    Some(l) => (DiagnosticSeverity::Error, Some(l.at)),
                    None => (DiagnosticSeverity::Warning, tcfg.map(|c| c.at)),
                };
                diags.push(diag(
                    severity,
                    format!(
                        "table `{}`: creating objects is disabled because `{}` is NOT NULL, has no default and is not editable or create-only",
                        table.name, f.name
                    ),
                    at,
                    Some(table.name.as_str()),
                    Some(f.name.as_str()),
                ));
            }
        }
        let can_delete = editable
            && tcfg
                .and_then(|c| c.allow_delete.as_ref())
                .is_some_and(|l| l.value);

        let referenced_by = reverse_relations(
            schema,
            ti,
            &junctions,
            &hidden_tables,
            &all_fields,
            &display_names,
        );
        let many_to_many = sides(
            schema,
            ti,
            &relations,
            &hidden_tables,
            &infos,
            &display_names,
        );

        tables.push(Some(ResolvedTable {
            schema_index: ti,
            name: table.name.as_str().to_owned(),
            display_name: display_names[ti].clone(),
            description: tcfg.and_then(|c| c.description.clone()),
            object_name: tcfg.and_then(|c| c.object_name.clone()),
            mode: info.mode,
            can_create,
            can_delete,
            has_triggers: table.has_triggers,
            identity: table.identity.clone(),
            fields: display.iter().map(|&c| fields[c].clone()).collect(),
            hidden_columns: (0..fields.len())
                .filter(|&c| fields[c].mode == FieldMode::Hidden)
                .collect(),
            label,
            search,
            order,
            referenced_by,
            many_to_many,
        }));
    }

    diags.sort_by(|a, b| {
        (a.line.is_none(), a.line, &a.table, &a.column).cmp(&(
            b.line.is_none(),
            b.line,
            &b.table,
            &b.column,
        ))
    });
    Model {
        schema: schema.clone(),
        tables,
        diagnostics: diags,
    }
}

/// Foreign keys into table `ti` from visible tables, except those of
/// junctions. A key whose referencing column is hidden is left out, so its
/// name cannot leak.
fn reverse_relations(
    schema: &Schema,
    ti: usize,
    junctions: &[Option<Junction<'_>>],
    hidden_tables: &[bool],
    fields: &[Vec<ResolvedField>],
    display_names: &[String],
) -> Vec<ReverseRelation> {
    schema
        .referencing(ti)
        .filter(|(ri, fk)| {
            junctions[*ri].is_none()
                && !hidden_tables[*ri]
                && fk
                    .from
                    .iter()
                    .all(|&c| fields[*ri][c].mode != FieldMode::Hidden)
        })
        .map(|(ri, fk)| {
            let t = &schema.tables[ri];
            ReverseRelation {
                relation: format!("fk:{}:{}", t.name, fk.id),
                table: ri,
                table_name: t.name.as_str().to_owned(),
                table_display_name: display_names[ri].clone(),
                fk_index: t
                    .foreign_keys
                    .iter()
                    .position(|f| f.id == fk.id)
                    .unwrap_or(0),
                columns: fk.from.clone(),
                via: fk
                    .from
                    .iter()
                    .map(|&c| fields[ri][c].display_name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            }
        })
        .collect()
}

/// The checklists shown on table `ti`.
fn sides(
    schema: &Schema,
    ti: usize,
    relations: &[Relation],
    hidden_tables: &[bool],
    infos: &[TableInfo],
    display_names: &[String],
) -> Vec<M2mSide> {
    let mut out = Vec::new();
    for r in relations {
        if r.mode == RelationMode::Hidden {
            continue;
        }
        for (this, other) in [(r.a, r.b), (r.b, r.a)] {
            if this.table != ti || hidden_tables[other.table] {
                continue;
            }
            let editable = r.mode == RelationMode::Editable
                && r.can_add
                && infos[ti].mode == TableMode::Editable;
            let mut link_fields = r.link_fields.clone();
            if !editable {
                for f in &mut link_fields {
                    if f.mode != FieldMode::Hidden {
                        f.mode = FieldMode::ReadOnly;
                    }
                }
            }
            let junction = &schema.tables[r.junction];
            out.push(M2mSide {
                relation: format!("m2m:{}", junction.name),
                junction: r.junction,
                junction_name: junction.name.as_str().to_owned(),
                junction_identity: junction.identity.clone(),
                this_column: this.column,
                this_key: this.key,
                other_column: other.column,
                other_key: other.key,
                other_table: other.table,
                other_table_name: schema.tables[other.table].name.as_str().to_owned(),
                heading: r
                    .headings
                    .iter()
                    .find(|(t, _)| *t == ti)
                    .map_or_else(|| display_names[other.table].clone(), |(_, h)| h.clone()),
                mode: if editable {
                    RelationMode::Editable
                } else {
                    RelationMode::ReadOnly
                },
                can_add: r.can_add,
                link_fields,
            });
        }
    }
    out
}
