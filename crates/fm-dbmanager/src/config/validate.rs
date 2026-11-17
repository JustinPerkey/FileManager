//! Diagnostics collection, value readers and the rules that apply to one value
//! on its own (label templates, patterns, bound ordering).

use std::cmp::Ordering;

use toml::de::{DeString, DeTable, DeValue};
use toml::Spanned;

use super::duration;
use super::model::{At, Bound, ConfigDiagnostic, DiagnosticSeverity, LabelPart, LabelTemplate};

pub(super) type Val<'i> = Spanned<DeValue<'i>>;
pub(super) type Key<'i> = Spanned<DeString<'i>>;

/// Collects diagnostics and maps byte offsets to 1-based line and column (in
/// characters).
pub(super) struct Ctx<'a> {
    text: &'a str,
    pub diags: Vec<ConfigDiagnostic>,
    /// How many errors so far; entries compare it before and after.
    pub errors: usize,
}

impl<'a> Ctx<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        Ctx {
            text,
            diags: Vec::new(),
            errors: 0,
        }
    }

    pub(super) fn at(&self, offset: usize) -> At {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let before = &self.text[..offset];
        let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        let col = before[line_start..].chars().count() + 1;
        let to_u32 = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        At {
            line: to_u32(line),
            col: to_u32(col),
        }
    }

    fn push(&mut self, severity: DiagnosticSeverity, offset: usize, message: String) {
        let at = self.at(offset);
        self.diags.push(ConfigDiagnostic {
            severity,
            message,
            line: Some(at.line),
            col: Some(at.col),
            table: None,
            column: None,
        });
    }

    pub(super) fn error(&mut self, offset: usize, message: String) {
        self.errors += 1;
        self.push(DiagnosticSeverity::Error, offset, message);
    }

    pub(super) fn warning(&mut self, offset: usize, message: String) {
        self.push(DiagnosticSeverity::Warning, offset, message);
    }

    pub(super) fn into_sorted(mut self) -> Vec<ConfigDiagnostic> {
        self.diags.sort_by_key(|d| (d.line, d.col));
        self.diags
    }
}

/// A table's entries in source order.
pub(super) fn sorted<'a, 'i>(table: &'a DeTable<'i>) -> Vec<(&'a Key<'i>, &'a Val<'i>)> {
    let mut v: Vec<_> = table.iter().collect();
    v.sort_by_key(|(k, _)| k.span().start);
    v
}

pub(super) fn key_name<'a>(key: &'a Key<'_>) -> &'a str {
    key.get_ref()
}

fn wrong_type(ctx: &mut Ctx<'_>, scope: &str, key: &str, want: &str, v: &Val<'_>) {
    ctx.error(
        v.span().start,
        format!(
            "{scope}: `{key}` must be {want}, found {}",
            v.get_ref().type_str()
        ),
    );
}

pub(super) fn expect_str<'a>(
    ctx: &mut Ctx<'_>,
    scope: &str,
    key: &str,
    v: &'a Val<'_>,
) -> Option<&'a str> {
    let s = v.get_ref().as_str();
    if s.is_none() {
        wrong_type(ctx, scope, key, "a string", v);
    }
    s
}

/// A string that is not empty or all whitespace.
pub(super) fn expect_text(
    ctx: &mut Ctx<'_>,
    scope: &str,
    key: &str,
    v: &Val<'_>,
) -> Option<String> {
    let s = expect_str(ctx, scope, key, v)?;
    if s.trim().is_empty() {
        ctx.error(
            v.span().start,
            format!("{scope}: `{key}` must not be empty"),
        );
        return None;
    }
    Some(s.to_string())
}

pub(super) fn expect_bool(ctx: &mut Ctx<'_>, scope: &str, key: &str, v: &Val<'_>) -> Option<bool> {
    let b = v.get_ref().as_bool();
    if b.is_none() {
        wrong_type(ctx, scope, key, "true or false", v);
    }
    b
}

fn int_of(v: &Val<'_>) -> Option<Result<i64, ()>> {
    match v.get_ref() {
        DeValue::Integer(i) => Some(i64::from_str_radix(i.as_str(), i.radix()).map_err(|_| ())),
        _ => None,
    }
}

pub(super) fn expect_int(ctx: &mut Ctx<'_>, scope: &str, key: &str, v: &Val<'_>) -> Option<i64> {
    match int_of(v) {
        Some(Ok(n)) => Some(n),
        Some(Err(())) => {
            ctx.error(
                v.span().start,
                format!("{scope}: `{key}` is out of range for a 64-bit integer"),
            );
            None
        }
        None => {
            wrong_type(ctx, scope, key, "an integer", v);
            None
        }
    }
}

pub(super) fn expect_table<'a, 'i>(
    ctx: &mut Ctx<'_>,
    scope: &str,
    key: &str,
    v: &'a Val<'i>,
) -> Option<&'a DeTable<'i>> {
    let t = v.get_ref().as_table();
    if t.is_none() {
        wrong_type(ctx, scope, key, "a table", v);
    }
    t
}

pub(super) fn expect_array<'a, 'i>(
    ctx: &mut Ctx<'_>,
    scope: &str,
    key: &str,
    v: &'a Val<'i>,
) -> Option<&'a toml::de::DeArray<'i>> {
    let a = v.get_ref().as_array();
    if a.is_none() {
        wrong_type(ctx, scope, key, "an array", v);
    }
    a
}

/// Reads `min` or `max`: an integer, a float, or a duration string.
pub(super) fn read_bound(ctx: &mut Ctx<'_>, scope: &str, key: &str, v: &Val<'_>) -> Option<Bound> {
    match v.get_ref() {
        DeValue::Integer(_) => expect_int(ctx, scope, key, v).map(Bound::Int),
        DeValue::Float(f) => match f.as_str().parse::<f64>() {
            Ok(x) if x.is_finite() => Some(Bound::Float(x)),
            _ => {
                ctx.error(v.span().start, format!("{scope}: `{key}` must be finite"));
                None
            }
        },
        DeValue::String(s) => {
            if let Err(why) = duration::parse(s) {
                ctx.error(
                    v.span().start,
                    format!("{scope}: `{key}` is not a valid duration: {why}"),
                );
                None
            } else {
                Some(Bound::Duration(s.to_string()))
            }
        }
        _ => {
            wrong_type(ctx, scope, key, "a number or a duration string", v);
            None
        }
    }
}

/// `Some(ordering)` when the two bounds have the same form and can be
/// compared. Different forms are only comparable against the field's resolved
/// kind, which needs the database.
#[expect(
    clippy::cast_precision_loss,
    reason = "bounds are compared as numbers; exactness beyond 2^53 does not matter for a config sanity check"
)]
pub(super) fn compare_bounds(lhs: &Bound, rhs: &Bound) -> Option<Ordering> {
    match (lhs, rhs) {
        (Bound::Int(l), Bound::Int(r)) => Some(l.cmp(r)),
        (Bound::Int(l), Bound::Float(r)) => (*l as f64).partial_cmp(r),
        (Bound::Float(l), Bound::Int(r)) => l.partial_cmp(&(*r as f64)),
        (Bound::Float(l), Bound::Float(r)) => l.partial_cmp(r),
        (Bound::Duration(l), Bound::Duration(r)) => {
            match (duration::parse(l).ok()?, duration::parse(r).ok()?) {
                (duration::Parsed::Count(l), duration::Parsed::Count(r))
                | (duration::Parsed::Millis(l), duration::Parsed::Millis(r)) => Some(l.cmp(&r)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Whether the whole value must match `pattern`, as the app anchors it.
pub(super) fn check_pattern(pattern: &str) -> Result<(), String> {
    regex::Regex::new(&format!("^(?:{pattern})$"))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Parses `{column}` templates. `{{` and `}}` are literal braces.
pub(super) fn parse_label(s: &str) -> Result<LabelTemplate, String> {
    let mut parts: Vec<LabelPart> = Vec::new();
    let mut literal = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                literal.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                literal.push('}');
            }
            '}' => {
                return Err("`}` without a matching `{` (write `}}` for a literal brace)".into())
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some('{') => {
                            return Err(
                                "`{` inside a placeholder (write `{{` for a literal brace)".into(),
                            );
                        }
                        Some(c) => name.push(c),
                        None => return Err("`{` is never closed".into()),
                    }
                }
                if name.trim().is_empty() {
                    return Err("empty placeholder `{}`".into());
                }
                if !literal.is_empty() {
                    parts.push(LabelPart::Literal(std::mem::take(&mut literal)));
                }
                parts.push(LabelPart::Column(name));
            }
            c => literal.push(c),
        }
    }
    if !literal.is_empty() {
        parts.push(LabelPart::Literal(literal));
    }
    Ok(LabelTemplate { parts })
}
