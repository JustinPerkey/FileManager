//! Walks the spanned TOML document into the config model. Every problem
//! becomes a located diagnostic; an entry with an error is rejected, never
//! half-applied.

use std::cmp::Ordering;
use std::collections::HashSet;

use toml::de::{DeTable, DeValue};

use super::model::{
    AllowedValues, Config, Defaults, DurationUnit, FieldConfig, FieldEntry, FieldMode, FieldType,
    Heading, Located, M2mConfig, M2mEntry, OrderBy, RelationMode, TableConfig, TableEntry,
    TableMode,
};
use super::validate::{
    check_pattern, compare_bounds, expect_array, expect_bool, expect_int, expect_str, expect_table,
    expect_text, key_name, parse_label, read_bound, sorted, Ctx, Key, Val,
};

const MODE_TABLE: &str = "`editable`, `read-only` or `hidden`";
const MODE_TABLE_ENTRY: &str = "`editable`, `read-only`, `hidden` or `pickable`";
const MODE_FIELD: &str = "`editable`, `read-only`, `hidden` or `create-only`";

/// Reads everything but `version`. Returns the config; unreadable parts have
/// already been reported on `ctx`.
pub(super) fn read(ctx: &mut Ctx<'_>, top: &DeTable<'_>) -> Config {
    let mut config = Config::default();
    // Set when a whole section is unusable: nothing may become more editable
    // than the author meant, so every default narrows.
    let mut narrow = false;
    for (key, value) in sorted(top) {
        match key_name(key) {
            "version" => {}
            "defaults" => match read_defaults(ctx, value) {
                Some(d) => config.defaults = d,
                None => narrow = true,
            },
            "tables" => {
                if !read_tables(ctx, value, &mut config.tables) {
                    narrow = true;
                }
            }
            "many_to_many" => {
                if !read_m2m(ctx, value, &mut config.many_to_many) {
                    narrow = true;
                }
            }
            other => {
                ctx.error(
                    key.span().start,
                    format!(
                        "unknown top-level key `{other}` (ignored; all defaults become read-only)"
                    ),
                );
                narrow = true;
            }
        }
    }
    if narrow {
        config.defaults = Defaults::read_only();
    }
    config
}

fn located<T>(ctx: &Ctx<'_>, key: &Key<'_>, value: T) -> Located<T> {
    Located {
        value,
        at: ctx.at(key.span().start),
    }
}

fn read_enum<T>(
    ctx: &mut Ctx<'_>,
    scope: &str,
    key: &str,
    value: &Val<'_>,
    parse: fn(&str) -> Option<T>,
    expected: &str,
) -> Option<T> {
    let s = expect_str(ctx, scope, key, value)?;
    let parsed = parse(s);
    if parsed.is_none() {
        ctx.error(
            value.span().start,
            format!("{scope}: `{key}` must be {expected}, found \"{s}\""),
        );
    }
    parsed
}

fn unknown_key(ctx: &mut Ctx<'_>, scope: &str, key: &Key<'_>) {
    ctx.error(
        key.span().start,
        format!("{scope}: unknown key `{}`", key_name(key)),
    );
}

/// `None` when `[defaults]` is invalid, so the caller narrows every default.
fn read_defaults(ctx: &mut Ctx<'_>, v: &Val<'_>) -> Option<Defaults> {
    let before = ctx.errors;
    let scope = "[defaults]";
    let table = expect_table(ctx, scope, "defaults", v)?;
    let mut d = Defaults::default();
    for (key, value) in sorted(table) {
        match key_name(key) {
            "table_mode" => {
                let allowed = |s: &str| TableMode::parse(s).filter(|m| *m != TableMode::Pickable);
                if let Some(m) = read_enum(
                    ctx,
                    scope,
                    "table_mode",
                    value,
                    allowed,
                    "`editable`, `read-only` or `hidden` (`pickable` is not a valid default)",
                ) {
                    d.table_mode = m;
                }
            }
            "field_mode" => {
                let allowed = |s: &str| FieldMode::parse(s).filter(|m| *m != FieldMode::CreateOnly);
                if let Some(m) = read_enum(
                    ctx,
                    scope,
                    "field_mode",
                    value,
                    allowed,
                    "`editable`, `read-only` or `hidden` (`create-only` is not a valid default)",
                ) {
                    d.field_mode = m;
                }
            }
            "relation_mode" => {
                if let Some(m) = read_enum(
                    ctx,
                    scope,
                    "relation_mode",
                    value,
                    RelationMode::parse,
                    MODE_TABLE,
                ) {
                    d.relation_mode = m;
                }
            }
            _ => unknown_key(ctx, scope, key),
        }
    }
    (ctx.errors == before).then_some(d)
}

fn read_tables(ctx: &mut Ctx<'_>, v: &Val<'_>, out: &mut Vec<TableEntry>) -> bool {
    let Some(table) = expect_table(ctx, "tables", "tables", v) else {
        return false;
    };
    for (key, value) in sorted(table) {
        out.push(read_table(ctx, key, value));
    }
    true
}

fn read_table(ctx: &mut Ctx<'_>, key: &Key<'_>, v: &Val<'_>) -> TableEntry {
    let name = key_name(key).to_string();
    let at = ctx.at(key.span().start);
    let scope = format!("table `{name}`");
    let Some(table) = expect_table(ctx, &scope, &name, v) else {
        return TableEntry::Rejected {
            name,
            at,
            hidden: false,
            fields: Vec::new(),
        };
    };
    let before = ctx.errors;
    let mut t = TableConfig {
        name: name.clone(),
        at,
        mode: None,
        display_name: None,
        description: None,
        object_name: None,
        label: None,
        order_by: Vec::new(),
        field_order: Vec::new(),
        allow_create: None,
        allow_delete: None,
        fields: Vec::new(),
    };
    let mut fields_value: Option<&Val<'_>> = None;
    for (k, value) in sorted(table) {
        let kn = key_name(k);
        match kn {
            "mode" => {
                if let Some(m) =
                    read_enum(ctx, &scope, kn, value, TableMode::parse, MODE_TABLE_ENTRY)
                {
                    t.mode = Some(located(ctx, k, m));
                }
            }
            "display_name" => t.display_name = expect_text(ctx, &scope, kn, value),
            "description" => t.description = expect_text(ctx, &scope, kn, value),
            "object_name" => t.object_name = expect_text(ctx, &scope, kn, value),
            "label" => {
                if let Some(s) = expect_str(ctx, &scope, kn, value) {
                    match parse_label(s) {
                        Ok(l) => t.label = Some(located(ctx, k, l)),
                        Err(why) => ctx.error(
                            value.span().start,
                            format!("{scope}: `label` is invalid: {why}"),
                        ),
                    }
                }
            }
            "order_by" => t.order_by = read_order_by(ctx, &scope, value),
            "field_order" => t.field_order = read_field_order(ctx, &scope, value),
            "allow_create" => {
                if let Some(b) = expect_bool(ctx, &scope, kn, value) {
                    t.allow_create = Some(located(ctx, k, b));
                }
            }
            "allow_delete" => {
                if let Some(b) = expect_bool(ctx, &scope, kn, value) {
                    t.allow_delete = Some(located(ctx, k, b));
                }
            }
            "fields" => {
                if expect_table(ctx, &scope, kn, value).is_some() {
                    fields_value = Some(value);
                }
            }
            _ => unknown_key(ctx, &scope, k),
        }
    }
    let table_ok = ctx.errors == before;
    // Field errors are reported either way but never reject the table.
    if let Some(fv) = fields_value {
        if let Some(ft) = fv.get_ref().as_table() {
            for (fk, fval) in sorted(ft) {
                t.fields.push(read_field(ctx, &name, fk, fval));
            }
        }
    }
    if table_ok {
        TableEntry::Valid(t)
    } else {
        TableEntry::Rejected {
            name,
            at,
            hidden: matches!(
                t.mode,
                Some(Located {
                    value: TableMode::Hidden,
                    ..
                })
            ),
            fields: t.fields,
        }
    }
}

fn read_order_by(ctx: &mut Ctx<'_>, scope: &str, v: &Val<'_>) -> Vec<OrderBy> {
    let mut out = Vec::new();
    let Some(arr) = expect_array(ctx, scope, "order_by", v) else {
        return out;
    };
    for el in arr {
        let at = ctx.at(el.span().start);
        match el.get_ref() {
            DeValue::String(s) => {
                if s.trim().is_empty() {
                    ctx.error(
                        el.span().start,
                        format!("{scope}: `order_by` has an empty column name"),
                    );
                } else {
                    out.push(OrderBy {
                        column: s.to_string(),
                        descending: false,
                        at,
                    });
                }
            }
            DeValue::Table(t) => {
                let mut column = None;
                let mut descending = false;
                let mut bad = false;
                for (k, x) in sorted(t) {
                    match key_name(k) {
                        "column" => match expect_text(ctx, scope, "column", x) {
                            Some(c) => column = Some(c),
                            None => bad = true,
                        },
                        "descending" => match expect_bool(ctx, scope, "descending", x) {
                            Some(b) => descending = b,
                            None => bad = true,
                        },
                        _ => {
                            ctx.error(
                                k.span().start,
                                format!(
                                    "{scope}: unknown key `{}` in `order_by` (use `column` and `descending`)",
                                    key_name(k)
                                ),
                            );
                            bad = true;
                        }
                    }
                }
                match column {
                    Some(column) if !bad => out.push(OrderBy {
                        column,
                        descending,
                        at,
                    }),
                    Some(_) => {}
                    None => {
                        if !bad {
                            ctx.error(
                                el.span().start,
                                format!("{scope}: an `order_by` table needs a `column`"),
                            );
                        }
                    }
                }
            }
            other => ctx.error(
                el.span().start,
                format!(
                    "{scope}: `order_by` elements must be a string or a table, found {}",
                    other.type_str()
                ),
            ),
        }
    }
    out
}

fn read_field_order(ctx: &mut Ctx<'_>, scope: &str, v: &Val<'_>) -> Vec<Located<String>> {
    let mut out = Vec::new();
    let Some(arr) = expect_array(ctx, scope, "field_order", v) else {
        return out;
    };
    for el in arr {
        if let Some(s) = expect_text(ctx, scope, "field_order", el) {
            out.push(Located {
                value: s,
                at: ctx.at(el.span().start),
            });
        }
    }
    out
}

fn read_field(ctx: &mut Ctx<'_>, table: &str, key: &Key<'_>, v: &Val<'_>) -> FieldEntry {
    let name = key_name(key).to_string();
    let at = ctx.at(key.span().start);
    let scope = format!("table `{table}`, field `{name}`");
    let Some(t) = expect_table(ctx, &scope, &name, v) else {
        return FieldEntry::Rejected {
            name,
            at,
            hidden: false,
        };
    };
    let before = ctx.errors;
    let mut f = FieldConfig {
        name: name.clone(),
        at,
        mode: None,
        display_name: None,
        help: None,
        field_type: None,
        unit: None,
        nullable: None,
        min: None,
        max: None,
        min_length: None,
        max_length: None,
        pattern: None,
        allowed: None,
        empty_is_null: None,
        multiline: None,
    };
    let mut marks = Marks::default();
    for (k, value) in sorted(t) {
        read_field_key(ctx, &scope, &mut f, &mut marks, k, value);
    }
    cross_checks(ctx, &scope, &f, &marks);
    if ctx.errors == before {
        FieldEntry::Valid(f)
    } else {
        FieldEntry::Rejected {
            name,
            at,
            hidden: matches!(
                f.mode,
                Some(Located {
                    value: FieldMode::Hidden,
                    ..
                })
            ),
        }
    }
}

/// Where the keys that cross-checks report at were written.
#[derive(Clone, Copy, Default)]
struct Marks {
    type_seen: bool,
    unit_at: usize,
    min_at: usize,
    len_at: usize,
}

fn read_field_key(
    ctx: &mut Ctx<'_>,
    scope: &str,
    f: &mut FieldConfig,
    marks: &mut Marks,
    k: &Key<'_>,
    value: &Val<'_>,
) {
    let kn = key_name(k);
    match kn {
        "mode" => {
            if let Some(m) = read_enum(ctx, scope, kn, value, FieldMode::parse, MODE_FIELD) {
                f.mode = Some(located(ctx, k, m));
            }
        }
        "display_name" => f.display_name = expect_text(ctx, scope, kn, value),
        "help" => f.help = expect_text(ctx, scope, kn, value),
        "type" => {
            marks.type_seen = true;
            if let Some(x) = read_enum(
                ctx,
                scope,
                kn,
                value,
                FieldType::parse,
                "`integer`, `real`, `text`, `bool` or `duration`",
            ) {
                f.field_type = Some(located(ctx, k, x));
            }
        }
        "unit" => {
            marks.unit_at = k.span().start;
            if let Some(x) = read_enum(
                ctx,
                scope,
                kn,
                value,
                DurationUnit::parse,
                "`milliseconds`, `seconds`, `minutes`, `hours` or `days`",
            ) {
                f.unit = Some(located(ctx, k, x));
            }
        }
        "nullable" => {
            if let Some(b) = expect_bool(ctx, scope, kn, value) {
                f.nullable = Some(located(ctx, k, b));
            }
        }
        "empty_is_null" => {
            if let Some(b) = expect_bool(ctx, scope, kn, value) {
                f.empty_is_null = Some(located(ctx, k, b));
            }
        }
        "multiline" => {
            if let Some(b) = expect_bool(ctx, scope, kn, value) {
                f.multiline = Some(located(ctx, k, b));
            }
        }
        _ => read_field_limit(ctx, scope, f, marks, k, value),
    }
}

/// The keys that bound a field's values.
fn read_field_limit(
    ctx: &mut Ctx<'_>,
    scope: &str,
    f: &mut FieldConfig,
    marks: &mut Marks,
    k: &Key<'_>,
    value: &Val<'_>,
) {
    let kn = key_name(k);
    match kn {
        "min" => {
            marks.min_at = k.span().start;
            if let Some(b) = read_bound(ctx, scope, kn, value) {
                f.min = Some(located(ctx, k, b));
            }
        }
        "max" => {
            if let Some(b) = read_bound(ctx, scope, kn, value) {
                f.max = Some(located(ctx, k, b));
            }
        }
        "min_length" | "max_length" => {
            let Some(n) = read_length(ctx, scope, kn, value) else {
                return;
            };
            if kn == "min_length" {
                marks.len_at = k.span().start;
                f.min_length = Some(located(ctx, k, n));
            } else {
                f.max_length = Some(located(ctx, k, n));
            }
        }
        "pattern" => {
            if let Some(s) = expect_str(ctx, scope, kn, value) {
                match check_pattern(s) {
                    Ok(()) => f.pattern = Some(located(ctx, k, s.to_string())),
                    Err(why) => ctx.error(
                        value.span().start,
                        format!("{scope}: `pattern` is not a valid regular expression: {why}"),
                    ),
                }
            }
        }
        "allowed" => {
            if let Some(a) = read_allowed(ctx, scope, value) {
                f.allowed = Some(located(ctx, k, a));
            }
        }
        _ => unknown_key(ctx, scope, k),
    }
}

/// Rules that relate two keys of one field.
fn cross_checks(ctx: &mut Ctx<'_>, scope: &str, f: &FieldConfig, marks: &Marks) {
    let Marks {
        type_seen,
        unit_at,
        min_at,
        len_at,
    } = *marks;
    let is_duration = matches!(
        f.field_type,
        Some(Located {
            value: FieldType::Duration,
            ..
        })
    );
    // An invalid `type` was already reported; do not pile a second error on it.
    if f.unit.is_some() && !is_duration && (f.field_type.is_some() || !type_seen) {
        ctx.error(
            unit_at,
            format!("{scope}: `unit` needs `type = \"duration\"`"),
        );
    }
    if let (Some(min), Some(max)) = (&f.min, &f.max) {
        if compare_bounds(&min.value, &max.value) == Some(Ordering::Greater) {
            ctx.error(min_at, format!("{scope}: `min` is greater than `max`"));
        }
    }
    if let (Some(min), Some(max)) = (&f.min_length, &f.max_length) {
        if min.value > max.value {
            ctx.error(
                len_at,
                format!("{scope}: `min_length` is greater than `max_length`"),
            );
        }
    }
}

fn read_length(ctx: &mut Ctx<'_>, scope: &str, key: &str, v: &Val<'_>) -> Option<u32> {
    let n = expect_int(ctx, scope, key, v)?;
    if let Ok(n) = u32::try_from(n) {
        return Some(n);
    }
    let why = if n < 0 {
        "must not be negative"
    } else {
        "is too large (the limit is 4294967295)"
    };
    ctx.error(v.span().start, format!("{scope}: `{key}` {why}"));
    None
}

fn read_allowed(ctx: &mut Ctx<'_>, scope: &str, v: &Val<'_>) -> Option<AllowedValues> {
    let arr = expect_array(ctx, scope, "allowed", v)?;
    if arr.is_empty() {
        ctx.error(
            v.span().start,
            format!("{scope}: `allowed` must not be empty"),
        );
        return None;
    }
    let before = ctx.errors;
    let mut ints: Vec<(i64, Option<String>)> = Vec::new();
    let mut texts: Vec<(String, Option<String>)> = Vec::new();
    let mut seen_int = HashSet::new();
    let mut seen_text = HashSet::new();
    for el in arr {
        let (value, label) = match el.get_ref() {
            DeValue::Table(t) => {
                let mut value = None;
                let mut label = None;
                for (k, x) in sorted(t) {
                    match key_name(k) {
                        "value" => value = Some(x),
                        "label" => label = expect_text(ctx, scope, "label", x),
                        _ => ctx.error(
                            k.span().start,
                            format!(
                                "{scope}: unknown key `{}` in an `allowed` entry (use `value` and `label`)",
                                key_name(k)
                            ),
                        ),
                    }
                }
                let Some(value) = value else {
                    ctx.error(
                        el.span().start,
                        format!("{scope}: an `allowed` entry needs a `value`"),
                    );
                    continue;
                };
                (value, label)
            }
            _ => (el, None),
        };
        match value.get_ref() {
            DeValue::Integer(_) => {
                if let Some(n) = expect_int(ctx, scope, "allowed", value) {
                    if seen_int.insert(n) {
                        ints.push((n, label));
                    } else {
                        ctx.error(
                            value.span().start,
                            format!("{scope}: `allowed` lists {n} twice"),
                        );
                    }
                }
            }
            DeValue::String(s) => {
                if seen_text.insert(s.to_string()) {
                    texts.push((s.to_string(), label));
                } else {
                    ctx.error(
                        value.span().start,
                        format!("{scope}: `allowed` lists \"{s}\" twice"),
                    );
                }
            }
            other => ctx.error(
                value.span().start,
                format!(
                    "{scope}: `allowed` values must be strings or integers, found {}",
                    other.type_str()
                ),
            ),
        }
    }
    if !ints.is_empty() && !texts.is_empty() {
        ctx.error(
            v.span().start,
            format!("{scope}: `allowed` mixes strings and integers"),
        );
    }
    if ctx.errors != before {
        return None;
    }
    Some(if texts.is_empty() {
        AllowedValues::Integers(ints)
    } else {
        AllowedValues::Texts(texts)
    })
}

fn read_m2m(ctx: &mut Ctx<'_>, v: &Val<'_>, out: &mut Vec<M2mEntry>) -> bool {
    let Some(table) = expect_table(ctx, "many_to_many", "many_to_many", v) else {
        return false;
    };
    for (key, value) in sorted(table) {
        out.push(read_m2m_entry(ctx, key, value));
    }
    true
}

fn read_m2m_entry(ctx: &mut Ctx<'_>, key: &Key<'_>, v: &Val<'_>) -> M2mEntry {
    let name = key_name(key).to_string();
    let at = ctx.at(key.span().start);
    let scope = format!("many-to-many `{name}`");
    let Some(table) = expect_table(ctx, &scope, &name, v) else {
        return M2mEntry::Rejected { name, at };
    };
    let before = ctx.errors;
    let mut m = M2mConfig {
        name: name.clone(),
        at,
        enabled: None,
        mode: None,
        columns: None,
        headings: Vec::new(),
    };
    for (k, value) in sorted(table) {
        let kn = key_name(k);
        match kn {
            "enabled" => {
                if let Some(b) = expect_bool(ctx, &scope, kn, value) {
                    m.enabled = Some(located(ctx, k, b));
                }
            }
            "mode" => {
                if let Some(x) = read_enum(ctx, &scope, kn, value, RelationMode::parse, MODE_TABLE)
                {
                    m.mode = Some(located(ctx, k, x));
                }
            }
            "columns" => {
                if let Some(c) = read_columns(ctx, &scope, value) {
                    m.columns = Some(located(ctx, k, c));
                }
            }
            "headings" => m.headings = read_headings(ctx, &scope, value),
            _ => unknown_key(ctx, &scope, k),
        }
    }
    if ctx.errors == before {
        M2mEntry::Valid(m)
    } else {
        M2mEntry::Rejected { name, at }
    }
}

fn read_columns(ctx: &mut Ctx<'_>, scope: &str, v: &Val<'_>) -> Option<[String; 2]> {
    let arr = expect_array(ctx, scope, "columns", v)?;
    let mut names = Vec::new();
    for el in arr {
        names.push(expect_text(ctx, scope, "columns", el)?);
    }
    match <[String; 2]>::try_from(names) {
        Ok(pair) if pair[0] != pair[1] => Some(pair),
        _ => {
            ctx.error(
                v.span().start,
                format!("{scope}: `columns` must be exactly two different column names"),
            );
            None
        }
    }
}

fn read_headings(ctx: &mut Ctx<'_>, scope: &str, v: &Val<'_>) -> Vec<Heading> {
    let mut out = Vec::new();
    let Some(table) = expect_table(ctx, scope, "headings", v) else {
        return out;
    };
    for (k, value) in sorted(table) {
        if let Some(text) = expect_text(ctx, scope, "headings", value) {
            out.push(Heading {
                side: key_name(k).to_string(),
                text,
                at: ctx.at(k.span().start),
            });
        }
    }
    out
}
