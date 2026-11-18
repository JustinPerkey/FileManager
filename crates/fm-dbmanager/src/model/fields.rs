//! Resolving one column into a field: kind, mode, rule, link.

use std::cmp::Ordering;

use super::rule::{real_text, scan_pattern};
use super::{diag, ResolvedField, ResolvedForeignKey};
use crate::config::{
    AllowedValues, At, Bound, ConfigDiagnostic, Defaults, DiagnosticSeverity, DurationUnit,
    FieldConfig, FieldMode, Located,
};
use crate::rules::{
    duration::{self, DurationError},
    infer_kind, kind_of_type, parse_default, type_override_allowed, AllowedValue, FieldKind,
    FieldRule,
};
use crate::schema::{Schema, Table};
use crate::value::DbValue;

pub(super) struct Env<'a> {
    pub schema: &'a Schema,
    pub defaults: Defaults,
    /// Parallel to `schema.tables`: the table's mode is `hidden`.
    pub hidden_tables: &'a [bool],
    pub display_names: &'a [String],
}

/// What the table the field belongs to does to it.
pub(super) struct TableCtx {
    /// Every field of the table is read-only.
    pub read_only: bool,
    /// An explicit `editable`/`create-only` on a field is worth a warning.
    pub warn_read_only: bool,
}

pub(super) enum Source<'a> {
    /// A valid entry, or none. `None` means the field is not mentioned.
    Config(Option<&'a FieldConfig>),
    /// A rejected field entry, or any field of a rejected table: only
    /// `hidden` survives.
    Narrowed { hidden: bool },
}

struct Sink<'a> {
    diags: &'a mut Vec<ConfigDiagnostic>,
    table: &'a str,
    column: &'a str,
    bad: bool,
}

impl Sink<'_> {
    fn push(&mut self, severity: DiagnosticSeverity, at: At, message: &str) {
        self.diags.push(diag(
            severity,
            format!("table `{}`, field `{}`: {message}", self.table, self.column),
            Some(at),
            Some(self.table),
            Some(self.column),
        ));
    }

    fn error(&mut self, at: At, message: &str) {
        self.bad = true;
        self.push(DiagnosticSeverity::Error, at, message);
    }

    fn warning(&mut self, at: At, message: &str) {
        self.push(DiagnosticSeverity::Warning, at, message);
    }
}

struct FkInfo {
    index: usize,
    multi: bool,
    first: bool,
    is_key: bool,
    target_table: usize,
    target_columns: Vec<usize>,
}

fn fk_of(table: &Table, column: usize) -> Option<FkInfo> {
    table.foreign_keys.iter().enumerate().find_map(|(i, fk)| {
        let target = fk.target.as_ref()?;
        fk.from.contains(&column).then(|| FkInfo {
            index: i,
            multi: fk.from.len() > 1,
            first: fk.from.first() == Some(&column),
            is_key: target.is_key,
            target_table: target.table,
            target_columns: target.columns.clone(),
        })
    })
}

/// A bound's canonical text and a comparable value.
#[derive(Clone, Copy)]
enum Num {
    Int(i128),
    Float(f64),
}

fn num_cmp(a: Num, b: Num) -> Option<Ordering> {
    match (a, b) {
        (Num::Int(a), Num::Int(b)) => Some(a.cmp(&b)),
        (Num::Float(a), Num::Float(b)) => a.partial_cmp(&b),
        _ => None,
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a real bound is compared as an f64, as validate parses it"
)]
fn bound(
    sink: &mut Sink<'_>,
    key: &str,
    b: &Located<Bound>,
    kind: FieldKind,
    unit: Option<DurationUnit>,
) -> Option<(String, Num)> {
    let at = b.at;
    match (kind, &b.value) {
        (FieldKind::Integer, Bound::Int(n)) => Some((n.to_string(), Num::Int(i128::from(*n)))),
        (FieldKind::Integer, _) => {
            sink.error(
                at,
                &format!("`{key}` of an integer field must be a TOML integer"),
            );
            None
        }
        (FieldKind::Real, Bound::Int(n)) => Some((n.to_string(), Num::Float(*n as f64))),
        (FieldKind::Real, Bound::Float(x)) => Some((real_text(*x), Num::Float(*x))),
        (FieldKind::Duration, Bound::Int(n)) => {
            let unit = unit?;
            Some((duration::format(*n, unit), Num::Int(i128::from(*n))))
        }
        (FieldKind::Duration, Bound::Duration(text)) => {
            let unit = unit?;
            match duration::parse(text, unit) {
                Ok(count) => Some((duration::format(count, unit), Num::Int(i128::from(count)))),
                Err(DurationError::Precision) => {
                    sink.error(
                        at,
                        &format!("`{key}` is not a whole number of {}", unit.name()),
                    );
                    None
                }
                Err(DurationError::Grammar | DurationError::Overflow) => {
                    sink.error(
                        at,
                        &format!("`{key}` is not a valid duration for this field"),
                    );
                    None
                }
            }
        }
        (FieldKind::Real | FieldKind::Duration, _) => {
            sink.error(at, &format!("`{key}` has the wrong form for this field"));
            None
        }
        _ => {
            sink.error(
                at,
                &format!("`{key}` applies to integer, real and duration fields only"),
            );
            None
        }
    }
}

fn forced_reason(
    col: &crate::schema::Column,
    kind: FieldKind,
    fk: Option<&FkInfo>,
    hidden_tables: &[bool],
) -> Option<&'static str> {
    if col.is_rowid_alias {
        Some("a rowid alias, assigned by SQLite")
    } else if col.generated {
        Some("a generated column")
    } else if kind == FieldKind::Blob {
        Some("a blob column")
    } else if kind == FieldKind::Any {
        Some("an untyped column (set `type` to make it editable)")
    } else if let Some(fk) = fk {
        if fk.multi {
            Some("part of a multi-column foreign key")
        } else if hidden_tables[fk.target_table] {
            Some("a foreign key into a hidden table")
        } else if !fk.is_key {
            Some("a foreign key that does not reference a key")
        } else {
            None
        }
    } else {
        None
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one linear pass over the keys of a field entry; splitting it would scatter the shared diagnostic sink"
)]
pub(super) fn resolve_field(
    env: &Env<'_>,
    table_index: usize,
    column: usize,
    source: &Source<'_>,
    tctx: &TableCtx,
    diags: &mut Vec<ConfigDiagnostic>,
) -> ResolvedField {
    let table = &env.schema.tables[table_index];
    let col = &table.columns[column];
    let mut sink = Sink {
        diags,
        table: table.name.as_str(),
        column: col.name.as_str(),
        bad: false,
    };
    let cfg = match source {
        Source::Config(c) => *c,
        Source::Narrowed { .. } => None,
    };
    let fk = fk_of(table, column);
    let linked = fk.as_ref().is_some_and(|f| f.is_key);

    // Keys that describe the value (not the display) do not apply to a link.
    let mut apply = cfg;
    if let (Some(c), true) = (cfg, linked) {
        let keys: [(&str, Option<At>); 8] = [
            ("type", c.field_type.as_ref().map(|l| l.at)),
            ("unit", c.unit.as_ref().map(|l| l.at)),
            ("min", c.min.as_ref().map(|l| l.at)),
            ("max", c.max.as_ref().map(|l| l.at)),
            ("min_length", c.min_length.as_ref().map(|l| l.at)),
            ("max_length", c.max_length.as_ref().map(|l| l.at)),
            ("pattern", c.pattern.as_ref().map(|l| l.at)),
            ("allowed", c.allowed.as_ref().map(|l| l.at)),
        ];
        for (key, at) in keys {
            if let Some(at) = at {
                sink.error(
                    at,
                    &format!("`{key}` does not apply to a foreign key field"),
                );
            }
        }
        apply = None;
    }

    // Kind.
    let mut kind = infer_kind(col, table.strict);
    if let Some(Located { value: ty, at }) = apply.and_then(|c| c.field_type.as_ref()) {
        if type_override_allowed(col, table.strict, *ty) {
            kind = kind_of_type(*ty);
        } else {
            let decl = if col.declared_type.is_empty() {
                "no declared type"
            } else {
                col.declared_type.as_str()
            };
            sink.error(
                *at,
                &format!("`type` is not valid for a column with {decl}; its storage would not keep the value"),
            );
        }
    }
    let unit = (kind == FieldKind::Duration).then(|| {
        apply
            .and_then(|c| c.unit.as_ref())
            .map_or(DurationUnit::Seconds, |u| u.value)
    });

    // Nullability.
    let not_null = col.not_null || (table.without_rowid && col.pk_position.is_some());
    let mut nullable = !not_null;
    if let Some(n) = cfg.and_then(|c| c.nullable.as_ref()) {
        if n.value && not_null {
            sink.error(n.at, "`nullable = true` on a NOT NULL column");
        } else {
            nullable = n.value;
        }
    }

    // Bounds.
    let mut min = apply
        .and_then(|c| c.min.as_ref())
        .and_then(|b| bound(&mut sink, "min", b, kind, unit));
    let mut max = apply
        .and_then(|c| c.max.as_ref())
        .and_then(|b| bound(&mut sink, "max", b, kind, unit));
    if let (Some((_, lo)), Some((_, hi))) = (&min, &max) {
        if num_cmp(*lo, *hi) == Some(Ordering::Greater) {
            let at = apply
                .and_then(|c| c.min.as_ref())
                .map_or_else(At::default, |m| m.at);
            sink.error(at, "`min` is greater than `max`");
            min = None;
            max = None;
        }
    }

    // Lengths, pattern.
    let mut min_length = None;
    let mut max_length = None;
    let mut pattern = None;
    if let Some(c) = apply {
        let text = kind == FieldKind::Text;
        for (key, l) in [("min_length", &c.min_length), ("max_length", &c.max_length)] {
            if let Some(l) = l {
                if text {
                    if key == "min_length" {
                        min_length = Some(l.value);
                    } else {
                        max_length = Some(l.value);
                    }
                } else {
                    sink.error(l.at, &format!("`{key}` applies to text fields only"));
                }
            }
        }
        if let Some(p) = &c.pattern {
            if text {
                let scan = scan_pattern(&p.value);
                if let Some(short) = scan.shorthand {
                    sink.error(
                        p.at,
                        &format!(
                            "`pattern` uses the shorthand class `{short}`, which Rust and JavaScript read differently; write an explicit class: `[0-9]`, `[A-Za-z0-9_]`, `[ \\t\\r\\n]`"
                        ),
                    );
                } else if let Err(why) = regex::Regex::new(&format!("^(?:{})$", p.value)) {
                    sink.error(p.at, &format!("`pattern` does not compile: {why}"));
                } else {
                    if scan.dot {
                        sink.warning(
                            p.at,
                            "`pattern` has an unescaped `.`, which JavaScript reads differently (it excludes \\r, U+2028 and U+2029); write `[^\\n]` for any character",
                        );
                    }
                    pattern = Some(p.value.clone());
                }
            } else {
                sink.error(p.at, "`pattern` applies to text fields only");
            }
        }
    }

    // Allowed values.
    let mut allowed = None;
    if let Some(a) = apply.and_then(|c| c.allowed.as_ref()) {
        match (&a.value, kind) {
            (AllowedValues::Integers(v), FieldKind::Integer) => {
                allowed = Some(
                    v.iter()
                        .map(|(n, label)| AllowedValue {
                            value: DbValue::Integer(n.to_string()),
                            label: label.clone().unwrap_or_else(|| n.to_string()),
                        })
                        .collect(),
                );
            }
            (AllowedValues::Texts(v), FieldKind::Text) => {
                allowed = Some(
                    v.iter()
                        .map(|(s, label)| AllowedValue {
                            value: DbValue::Text(s.clone()),
                            label: label.clone().unwrap_or_else(|| s.clone()),
                        })
                        .collect(),
                );
            }
            _ => sink.error(
                a.at,
                "`allowed` needs an integer field with integer values or a text field with text values",
            ),
        }
    }

    // empty_is_null, multiline.
    let mut empty_is_null = nullable && kind == FieldKind::Text;
    if let Some(e) = cfg.and_then(|c| c.empty_is_null.as_ref()) {
        if nullable && kind == FieldKind::Text {
            empty_is_null = e.value;
        } else {
            sink.error(e.at, "`empty_is_null` needs a nullable text field");
        }
    }
    let mut multiline = false;
    if let Some(m) = cfg.and_then(|c| c.multiline.as_ref()) {
        if kind == FieldKind::Text {
            multiline = m.value;
        } else {
            sink.warning(m.at, "`multiline` applies to text fields only; ignored");
        }
    }

    // Mode.
    let explicit = cfg.and_then(|c| c.mode.as_ref());
    let (requested, is_explicit) =
        explicit.map_or((env.defaults.field_mode, false), |m| (m.value, true));
    let wants_write = matches!(requested, FieldMode::Editable | FieldMode::CreateOnly);
    let mut mode = match source {
        Source::Narrowed { hidden } => {
            if *hidden {
                FieldMode::Hidden
            } else {
                FieldMode::ReadOnly
            }
        }
        Source::Config(_) => {
            if requested == FieldMode::Hidden {
                FieldMode::Hidden
            } else if tctx.read_only {
                if let (Some(m), true, true) = (explicit, wants_write, tctx.warn_read_only) {
                    sink.warning(
                        m.at,
                        "the table is not editable, so the field stays read-only",
                    );
                }
                FieldMode::ReadOnly
            } else if let Some(reason) = forced_reason(col, kind, fk.as_ref(), env.hidden_tables) {
                if let (Some(m), true) = (explicit, wants_write) {
                    sink.error(
                        m.at,
                        &format!("the column is {reason}, so it is always read-only"),
                    );
                }
                FieldMode::ReadOnly
            } else if col.pk_position.is_some() && requested == FieldMode::Editable {
                if let (Some(m), true) = (explicit, is_explicit) {
                    sink.error(
                        m.at,
                        "a primary-key column cannot be edited after creation; use `create-only`",
                    );
                    FieldMode::ReadOnly
                } else {
                    FieldMode::CreateOnly
                }
            } else {
                requested
            }
        }
    };
    if sink.bad && mode != FieldMode::Hidden {
        mode = FieldMode::ReadOnly;
    }

    let has_default = col.default_sql.is_some();
    let default_value = col
        .default_sql
        .as_deref()
        .and_then(|s| parse_default(s, kind, unit));
    let foreign_key = fk
        .as_ref()
        .filter(|f| f.is_key && (!f.multi || f.first))
        .map(|f| {
            let hidden = env.hidden_tables[f.target_table];
            ResolvedForeignKey {
                fk_index: f.index,
                table: f.target_table,
                table_name: env.schema.tables[f.target_table].name.as_str().to_owned(),
                table_display_name: (!hidden).then(|| env.display_names[f.target_table].clone()),
                columns: f.target_columns.clone(),
                navigable: !hidden,
            }
        });

    ResolvedField {
        column,
        name: col.name.as_str().to_owned(),
        display_name: cfg
            .and_then(|c| c.display_name.clone())
            .unwrap_or_else(|| col.name.as_str().to_owned()),
        help: cfg.and_then(|c| c.help.clone()),
        mode,
        kind,
        declared_type: col.declared_type.clone(),
        rule: FieldRule {
            kind,
            nullable,
            unit,
            min: min.map(|m| m.0),
            max: max.map(|m| m.0),
            min_length,
            max_length,
            pattern,
            allowed,
            empty_is_null,
        },
        is_key: col.pk_position.is_some(),
        required_on_create: not_null && !has_default && !col.is_rowid_alias && !col.generated,
        has_default,
        default_value,
        multiline,
        foreign_key,
    }
}
