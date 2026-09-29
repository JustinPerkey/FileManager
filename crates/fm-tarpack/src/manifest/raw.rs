//! Reads the spanned TOML document into raw pieces. Unknown keys, wrong types
//! and missing fields become located diagnostics, collected with everything
//! else, instead of aborting the parse.

use serde::de::IntoDeserializer;
use serde::Deserialize;
use toml::de::{DeTable, DeValue};
use toml::Spanned;

use super::validate::{Ctx, Scope};

/// `normalize_eol` is deliberately absent: it is a per-file choice.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDefaults {
    pub mode: Option<Spanned<String>>,
    pub dir_mode: Option<Spanned<String>>,
    pub uid: Option<Spanned<i64>>,
    pub gid: Option<Spanned<i64>>,
    pub uname: Option<Spanned<String>>,
    pub gname: Option<Spanned<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawFile {
    pub id: Spanned<String>,
    pub source: Spanned<String>,
    pub dir: Spanned<String>,
    pub name: Option<Spanned<String>>,
    pub mode: Option<Spanned<String>>,
    pub uid: Option<Spanned<i64>>,
    pub gid: Option<Spanned<i64>>,
    pub uname: Option<Spanned<String>>,
    pub gname: Option<Spanned<String>>,
    pub normalize_eol: Option<Spanned<bool>>,
}

/// One `[[file]]` table. `id` is the raw string id when the table has one,
/// even if the table as a whole failed to deserialize.
pub struct RawFileSlot {
    /// 1-based position of the table.
    pub index: usize,
    pub id: Option<Spanned<String>>,
    /// `source` when it is a string.
    pub source: Option<String>,
    /// Start of the table: its `[[file]]` header, or the element of an inline
    /// array.
    pub at: usize,
    pub file: Option<RawFile>,
}

pub struct RawDoc {
    pub version: Option<Spanned<i64>>,
    pub name: Option<Spanned<String>>,
    pub output_name: Option<Spanned<String>>,
    pub defaults: Option<RawDefaults>,
    pub files: Vec<RawFileSlot>,
}

const TOP_KEYS: [&str; 5] = ["version", "name", "output_name", "defaults", "file"];
const DEFAULTS_KEYS: [&str; 6] = ["mode", "dir_mode", "uid", "gid", "uname", "gname"];
const FILE_KEYS: [&str; 10] = [
    "id",
    "source",
    "dir",
    "name",
    "mode",
    "uid",
    "gid",
    "uname",
    "gname",
    "normalize_eol",
];

/// Removes keys not in `allowed`, reporting each at the key's span.
fn strip_unknown<'i>(
    ctx: &mut Ctx,
    table: DeTable<'i>,
    allowed: &[&str],
    scope: Scope,
) -> DeTable<'i> {
    let mut kept = DeTable::new();
    for (key, value) in table {
        if allowed.contains(&key.get_ref().as_ref()) {
            kept.insert(key, value);
            continue;
        }
        let msg = if matches!(scope, Scope::Defaults) && key.get_ref() == "normalize_eol" {
            "`normalize_eol` is set per file, not in [defaults]".to_string()
        } else {
            format!("unknown key `{}`", key.get_ref())
        };
        ctx.error(scope, key.span().start, msg);
    }
    kept
}

fn take<'i>(table: &mut DeTable<'i>, name: &str) -> Option<Spanned<DeValue<'i>>> {
    let key = table.keys().find(|k| k.get_ref().as_ref() == name)?.clone();
    table.remove(&key)
}

/// Takes a required field. One that is absent is reported here; one that is
/// present but of the wrong type is reported by `de`.
fn required<'i>(
    ctx: &mut Ctx,
    table: &mut DeTable<'i>,
    key: &str,
    scope: Scope,
) -> Option<Spanned<DeValue<'i>>> {
    let v = take(table, key);
    if v.is_none() {
        ctx.error(scope, 0, format!("missing field `{key}`"));
    }
    v
}

fn find<'a, 'i>(table: &'a DeTable<'i>, name: &str) -> Option<&'a Spanned<DeValue<'i>>> {
    table
        .iter()
        .find(|(k, _)| k.get_ref().as_ref() == name)
        .map(|(_, v)| v)
}

/// Deserializes one piece, turning a failure into a located diagnostic.
fn de<'de, T: Deserialize<'de>>(
    ctx: &mut Ctx,
    value: Spanned<DeValue<'de>>,
    scope: Scope,
) -> Option<T> {
    let span = value.span();
    match T::deserialize(value.into_deserializer()) {
        Ok(v) => Some(v),
        Err(e) => {
            let at = e.span().unwrap_or(span).start;
            ctx.error(scope, at, e.message().trim().to_string());
            None
        }
    }
}

fn type_error(ctx: &mut Ctx, what: &str, value: &Spanned<DeValue<'_>>, scope: Scope) {
    ctx.error(
        scope,
        value.span().start,
        format!(
            "invalid type: {}, expected {what}",
            value.get_ref().type_str()
        ),
    );
}

pub fn read(ctx: &mut Ctx, doc: DeTable<'_>) -> RawDoc {
    // `version`, unknown keys and `file` withhold every entry; `name` and
    // `output_name` do not change how an entry is read.
    let hard = Scope::Top { withholds: true };
    let soft = Scope::Top { withholds: false };
    let mut table = strip_unknown(ctx, doc, &TOP_KEYS, hard);
    let version = required(ctx, &mut table, "version", hard).and_then(|v| de(ctx, v, hard));
    let name = required(ctx, &mut table, "name", soft).and_then(|v| de(ctx, v, soft));
    let output_name = take(&mut table, "output_name").and_then(|v| de(ctx, v, soft));
    let defaults_val = take(&mut table, "defaults");
    let file_val = take(&mut table, "file");

    let defaults = defaults_val.and_then(|v| match v.get_ref() {
        DeValue::Table(_) => {
            let span = v.span();
            let DeValue::Table(t) = v.into_inner() else {
                unreachable!()
            };
            let t = strip_unknown(ctx, t, &DEFAULTS_KEYS, Scope::Defaults);
            de(ctx, Spanned::new(span, DeValue::Table(t)), Scope::Defaults)
        }
        _ => {
            type_error(ctx, "a table", &v, Scope::Defaults);
            None
        }
    });

    let mut files = Vec::new();
    match file_val {
        None => {}
        Some(v) => {
            let span = v.span();
            match v.into_inner() {
                DeValue::Array(items) => {
                    for (i, item) in items.into_iter().enumerate() {
                        files.push(read_file(ctx, i + 1, item));
                    }
                }
                other => {
                    let v = Spanned::new(span, other);
                    type_error(ctx, "an array of tables", &v, hard);
                }
            }
        }
    }

    RawDoc {
        version,
        name,
        output_name,
        defaults,
        files,
    }
}

fn read_file(ctx: &mut Ctx, n: usize, item: Spanned<DeValue<'_>>) -> RawFileSlot {
    let span = item.span();
    let DeValue::Table(table) = item.get_ref() else {
        type_error(ctx, "a table", &item, Scope::file("", n));
        return RawFileSlot {
            index: n,
            id: None,
            source: None,
            at: span.start,
            file: None,
        };
    };
    let id = find(table, "id").and_then(|v| match v.get_ref() {
        DeValue::String(s) => Some(Spanned::new(v.span(), s.to_string())),
        _ => None,
    });
    let source = find(table, "source").and_then(|v| match v.get_ref() {
        DeValue::String(s) => Some(s.to_string()),
        _ => None,
    });
    let DeValue::Table(table) = item.into_inner() else {
        unreachable!()
    };
    let scope = Scope::file(id.as_ref().map_or("", |i| i.get_ref().as_str()), n);
    let table = strip_unknown(ctx, table, &FILE_KEYS, scope);
    let file = de(
        ctx,
        Spanned::new(span.clone(), DeValue::Table(table)),
        scope,
    );
    RawFileSlot {
        index: n,
        id,
        source,
        at: span.start,
        file,
    }
}
