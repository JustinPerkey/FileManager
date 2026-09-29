//! Reads the spanned TOML document into raw pieces. Unknown keys, wrong types
//! and missing fields become located diagnostics, collected with everything
//! else, instead of aborting the parse.

use serde::de::IntoDeserializer;
use serde::Deserialize;
use toml::de::{DeTable, DeValue};
use toml::Spanned;

use super::validate::Ctx;

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
    pub id: Option<Spanned<String>>,
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

/// Who a table's diagnostics are attributed to.
#[derive(Clone, Copy)]
enum Scope<'a> {
    Top,
    Defaults,
    File(Option<&'a str>, usize),
}

impl Scope<'_> {
    fn report(self, ctx: &mut Ctx, at: usize, message: String) {
        match self {
            Scope::Top => ctx.push_error(at, None, message),
            Scope::Defaults => ctx.push_error(at, None, format!("[defaults]: {message}")),
            Scope::File(Some(id), _) => ctx.push_error(at, Some(id), message),
            Scope::File(None, n) => ctx.push_error(at, None, format!("[[file]] #{n}: {message}")),
        }
    }
}

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
        scope.report(ctx, key.span().start, msg);
    }
    kept
}

fn take<'i>(table: &mut DeTable<'i>, name: &str) -> Option<Spanned<DeValue<'i>>> {
    let key = table.keys().find(|k| k.get_ref().as_ref() == name)?.clone();
    table.remove(&key)
}

/// Takes a required field. One that is absent is reported here; one that is
/// present but of the wrong type is reported by `de`.
fn required<'i>(ctx: &mut Ctx, table: &mut DeTable<'i>, key: &str) -> Option<Spanned<DeValue<'i>>> {
    let v = take(table, key);
    if v.is_none() {
        ctx.push_error(0, None, format!("missing field `{key}`"));
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
            scope.report(ctx, at, e.message().trim().to_string());
            None
        }
    }
}

fn type_error(ctx: &mut Ctx, what: &str, value: &Spanned<DeValue<'_>>, scope: Scope) {
    scope.report(
        ctx,
        value.span().start,
        format!(
            "invalid type: {}, expected {what}",
            value.get_ref().type_str()
        ),
    );
}

pub fn read(ctx: &mut Ctx, doc: DeTable<'_>) -> RawDoc {
    let mut table = strip_unknown(ctx, doc, &TOP_KEYS, Scope::Top);
    let version = required(ctx, &mut table, "version").and_then(|v| de(ctx, v, Scope::Top));
    let name = required(ctx, &mut table, "name").and_then(|v| de(ctx, v, Scope::Top));
    let output_name = take(&mut table, "output_name").and_then(|v| de(ctx, v, Scope::Top));
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
                    type_error(ctx, "an array of tables", &v, Scope::Top);
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
        type_error(ctx, "a table", &item, Scope::File(None, n));
        return RawFileSlot {
            id: None,
            file: None,
        };
    };
    let id = find(table, "id").and_then(|v| match v.get_ref() {
        DeValue::String(s) => Some(Spanned::new(v.span(), s.to_string())),
        _ => None,
    });
    let DeValue::Table(table) = item.into_inner() else {
        unreachable!()
    };
    let scope_id = id
        .as_ref()
        .map(|i| i.get_ref().as_str())
        .filter(|s| !s.is_empty());
    let scope = Scope::File(scope_id, n);
    let table = strip_unknown(ctx, table, &FILE_KEYS, scope);
    let file = de(ctx, Spanned::new(span, DeValue::Table(table)), scope);
    RawFileSlot { id, file }
}
