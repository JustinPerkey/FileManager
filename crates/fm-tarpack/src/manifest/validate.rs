//! Turns a raw manifest into a validated `Manifest`, collecting every error
//! and warning with its line and column.

use std::collections::HashMap;
use std::ops::Range;

use toml::Spanned;

use super::mode::parse_mode;
use super::model::{Diagnostic, Entry, Manifest, Owner, Severity};
use super::raw::{RawDefaults, RawDoc, RawFile};

const DEFAULT_MODE: u32 = 0o644;
const DEFAULT_DIR_MODE: u32 = 0o755;
const MAX_OWNER_NAME: usize = 32;
/// A stored path this long or longer needs a GNU long-name record.
const LONG_NAME_BYTES: usize = 100;

/// Maps byte offsets to 1-based line and column (in characters).
pub(super) struct LineIndex<'a> {
    text: &'a str,
}

impl<'a> LineIndex<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        LineIndex { text }
    }

    pub(super) fn locate(&self, offset: usize) -> (u32, u32) {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let before = &self.text[..offset];
        let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        let col = before[line_start..].chars().count() + 1;
        (line as u32, col as u32)
    }
}

pub(super) struct Ctx<'a> {
    index: LineIndex<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> Ctx<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        Ctx {
            index: LineIndex::new(text),
            diags: Vec::new(),
        }
    }

    /// An error at byte offset `at`. With an id, the message is prefixed
    /// ``file `<id>`: `` and the diagnostic names the entry; a message that
    /// already carries its own prefix is passed with `id` `None`.
    pub(super) fn push_error(&mut self, at: usize, id: Option<&str>, message: String) {
        self.push(Severity::Error, at, id, message);
    }

    fn push(&mut self, severity: Severity, at: usize, id: Option<&str>, message: String) {
        let (line, col) = self.index.locate(at);
        let message = match id {
            Some(id) => format!("file `{id}`: {message}"),
            None => message,
        };
        self.diags.push(Diagnostic {
            severity,
            line,
            col,
            entry_id: id.map(str::to_string),
            message,
        });
    }
    fn error(&mut self, span: &Range<usize>, id: Option<&str>, message: String) {
        self.push(Severity::Error, span.start, id, message);
    }
    fn warn(&mut self, span: &Range<usize>, id: Option<&str>, message: String) {
        self.push(Severity::Warning, span.start, id, message);
    }
}

/// Converts a TOML syntax error into a single diagnostic.
pub(super) fn syntax_error(text: &str, err: &toml::de::Error) -> Diagnostic {
    let (line, col) = err
        .span()
        .map_or((1, 1), |s| LineIndex::new(text).locate(s.start));
    Diagnostic {
        severity: Severity::Error,
        line,
        col,
        entry_id: None,
        message: err.message().trim().to_string(),
    }
}

fn bad_leaf(s: &str) -> Option<&'static str> {
    if s.is_empty() {
        Some("must not be empty")
    } else if s.contains(['/', '\\', '\0']) {
        Some("must not contain `/`, `\\` or NUL")
    } else if s == "." || s == ".." {
        Some("must not be `.` or `..`")
    } else {
        None
    }
}

fn bad_dir(dir: &str) -> Option<&'static str> {
    if !dir.starts_with('/') {
        return Some("must be absolute (start with `/`)");
    }
    if dir.contains(['\\', '\0']) {
        return Some("must not contain a backslash or NUL");
    }
    let rest = &dir[1..];
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    if rest.is_empty() {
        return None;
    }
    for seg in rest.split('/') {
        if seg == ".." {
            return Some("must not contain a `..` segment");
        }
        if seg == "." {
            return Some("must not contain a `.` segment");
        }
        if seg.is_empty() {
            return Some("must not contain an empty segment (`//`)");
        }
    }
    None
}

fn to_u32(ctx: &mut Ctx, field: &str, v: &Spanned<i64>, id: Option<&str>) -> Option<u32> {
    match u32::try_from(*v.get_ref()) {
        Ok(n) => Some(n),
        Err(_) => {
            ctx.error(
                &v.span(),
                id,
                format!("{field} `{}` does not fit in u32", v.get_ref()),
            );
            None
        }
    }
}

fn mode_field(ctx: &mut Ctx, field: &str, v: &Spanned<String>, id: Option<&str>) -> Option<u32> {
    match parse_mode(v.get_ref()) {
        Ok(m) => Some(m),
        Err(e) => {
            ctx.error(&v.span(), id, format!("{field}: {e}"));
            None
        }
    }
}

fn owner_name(ctx: &mut Ctx, field: &str, v: &Spanned<String>, id: Option<&str>) -> Option<String> {
    let s = v.get_ref();
    if s.is_empty() || s.len() > MAX_OWNER_NAME {
        ctx.error(
            &v.span(),
            id,
            format!(
                "{field} must be 1 to {MAX_OWNER_NAME} bytes, got {}",
                s.len()
            ),
        );
        None
    } else {
        Some(s.clone())
    }
}

struct Defaults {
    mode: u32,
    dir_mode: u32,
    owner: Owner,
}

fn resolve_defaults(ctx: &mut Ctx, raw: Option<&RawDefaults>) -> Defaults {
    let mut d = Defaults {
        mode: DEFAULT_MODE,
        dir_mode: DEFAULT_DIR_MODE,
        owner: Owner {
            uid: 0,
            gid: 0,
            uname: "root".into(),
            gname: "root".into(),
        },
    };
    let Some(raw) = raw else { return d };
    if let Some(v) = &raw.mode {
        d.mode = mode_field(ctx, "defaults.mode", v, None).unwrap_or(d.mode);
    }
    if let Some(v) = &raw.dir_mode {
        d.dir_mode = mode_field(ctx, "defaults.dir_mode", v, None).unwrap_or(d.dir_mode);
    }
    if let Some(v) = &raw.uid {
        d.owner.uid = to_u32(ctx, "defaults.uid", v, None).unwrap_or(d.owner.uid);
    }
    if let Some(v) = &raw.gid {
        d.owner.gid = to_u32(ctx, "defaults.gid", v, None).unwrap_or(d.owner.gid);
    }
    if let Some(v) = &raw.uname {
        if let Some(s) = owner_name(ctx, "defaults.uname", v, None) {
            d.owner.uname = s;
        }
    }
    if let Some(v) = &raw.gname {
        if let Some(s) = owner_name(ctx, "defaults.gname", v, None) {
            d.owner.gname = s;
        }
    }
    d
}

fn resolve_entry(ctx: &mut Ctx, f: &RawFile, defaults: &Defaults) -> Option<Entry> {
    let id_str = f.id.get_ref().as_str();
    let id = (!id_str.is_empty()).then_some(id_str);
    let mut ok = true;

    if id_str.is_empty() {
        ctx.error(&f.id.span(), None, "id must not be empty".into());
        ok = false;
    }

    let source = f.source.get_ref();
    if let Some(why) = bad_leaf(source) {
        ctx.error(&f.source.span(), id, format!("source {why}"));
        ok = false;
    }

    let (name, name_span) = match &f.name {
        Some(n) => (n.get_ref().as_str(), n.span()),
        None => (source.as_str(), f.source.span()),
    };
    if f.name.is_some() {
        if let Some(why) = bad_leaf(name) {
            ctx.error(&name_span, id, format!("name {why}"));
            ok = false;
        }
    }

    let dir = f.dir.get_ref();
    if let Some(why) = bad_dir(dir) {
        ctx.error(&f.dir.span(), id, format!("dir {why}"));
        ok = false;
    }

    let mut mode = defaults.mode;
    if let Some(v) = &f.mode {
        match mode_field(ctx, "mode", v, id) {
            Some(m) => mode = m,
            None => ok = false,
        }
    }
    let mut owner = defaults.owner.clone();
    if let Some(v) = &f.uid {
        match to_u32(ctx, "uid", v, id) {
            Some(n) => owner.uid = n,
            None => ok = false,
        }
    }
    if let Some(v) = &f.gid {
        match to_u32(ctx, "gid", v, id) {
            Some(n) => owner.gid = n,
            None => ok = false,
        }
    }
    if let Some(v) = &f.uname {
        match owner_name(ctx, "uname", v, id) {
            Some(s) => owner.uname = s,
            None => ok = false,
        }
    }
    if let Some(v) = &f.gname {
        match owner_name(ctx, "gname", v, id) {
            Some(s) => owner.gname = s,
            None => ok = false,
        }
    }
    let normalize_eol = f.normalize_eol.as_ref().is_some_and(|b| *b.get_ref());

    ok.then(|| {
        Entry::new(
            id_str.to_string(),
            source.clone(),
            dir.clone(),
            name.to_string(),
            mode,
            owner,
            normalize_eol,
        )
    })
}

/// Every directory a stored path needs: `dir` (trailing `/` trimmed) and each
/// ancestor of it except `/`.
fn needed_dirs(dir: &str) -> Vec<&str> {
    let dir = dir.trim_end_matches('/');
    let mut out = Vec::new();
    let mut end = dir.len();
    while end > 0 {
        out.push(&dir[..end]);
        end = dir[..end].rfind('/').unwrap_or(0);
    }
    out
}

pub(super) fn validate(
    mut ctx: Ctx,
    raw: RawDoc,
) -> Result<(Manifest, Vec<Diagnostic>), Vec<Diagnostic>> {
    match &raw.version {
        Some(v) if *v.get_ref() != 1 => ctx.error(
            &v.span(),
            None,
            format!(
                "unsupported version {}; this program reads version 1",
                v.get_ref()
            ),
        ),
        _ => {}
    }
    match &raw.name {
        Some(n) if n.get_ref().is_empty() => {
            ctx.error(&n.span(), None, "name must not be empty".into());
        }
        _ => {}
    }
    if let Some(o) = &raw.output_name {
        let s = o.get_ref();
        if s.is_empty() || s.contains(['/', '\\', '\0']) {
            ctx.error(
                &o.span(),
                None,
                "output_name must not be empty or contain `/`, `\\` or NUL".into(),
            );
        }
    }

    let defaults = resolve_defaults(&mut ctx, raw.defaults.as_ref());

    let mut entries: Vec<Entry> = Vec::new();
    // Raw file of each resolved entry, for locating later errors.
    let mut resolved: Vec<&RawFile> = Vec::new();
    let mut ids: HashMap<&str, usize> = HashMap::new();
    let mut targets: HashMap<String, &str> = HashMap::new();
    let mut sources: HashMap<String, &str> = HashMap::new();

    for slot in &raw.files {
        // The raw id takes part in the duplicate check even when the table
        // itself failed to deserialize.
        let id_str = slot.id.as_ref().map_or("", |i| i.get_ref().as_str());
        let id = (!id_str.is_empty()).then_some(id_str);

        if let (Some(id_val), Some(id)) = (&slot.id, id) {
            if let Some(&first) = ids.get(id) {
                let (line, _) = ctx.index.locate(first);
                ctx.error(
                    &id_val.span(),
                    Some(id),
                    format!("duplicate id (first used on line {line})"),
                );
            } else {
                ids.insert(id, id_val.span().start);
            }
        }

        let Some(file) = &slot.file else { continue };
        let Some(e) = resolve_entry(&mut ctx, file, &defaults) else {
            continue;
        };

        // Duplicate targets are reported at the `name` value, else at `dir`.
        let at = file
            .name
            .as_ref()
            .map_or_else(|| file.dir.span(), |n| n.span());
        let path = e.target_path();
        if let Some(prev) = targets.get(&path) {
            ctx.error(
                &at,
                id,
                format!("target path `{path}` is already used by file `{prev}`"),
            );
        } else {
            targets.insert(path.clone(), id_str);
        }
        if path.len() >= LONG_NAME_BYTES {
            ctx.warn(
                &file.dir.span(),
                id,
                format!(
                    "stored path `{path}` is 100 bytes or longer ({} bytes); \
                     the archive will use a GNU long-name record",
                    path.len()
                ),
            );
        }
        let key = e.source().to_lowercase();
        if let Some(prev) = sources.get(&key) {
            ctx.warn(
                &file.source.span(),
                id,
                format!(
                    "source `{}` matches file `{prev}` (compared case-insensitively); \
                     a dropped file cannot tell them apart, so pick their files per row",
                    e.source()
                ),
            );
        } else {
            sources.insert(key, id_str);
        }
        entries.push(e);
        resolved.push(file);
    }

    // A file's stored path must not also be a directory another entry needs.
    for (i, (a, fa)) in entries.iter().zip(&resolved).enumerate() {
        let path = a.target_path();
        for (j, b) in entries.iter().enumerate() {
            if i != j && needed_dirs(b.target_dir()).contains(&path.as_str()) {
                let at = fa.name.as_ref().map_or_else(|| fa.dir.span(), |n| n.span());
                ctx.error(
                    &at,
                    Some(a.id()),
                    format!(
                        "target path `{path}` is also a directory of file `{}`",
                        b.id()
                    ),
                );
            }
        }
    }

    if raw.files.is_empty() {
        ctx.push(
            Severity::Warning,
            0,
            None,
            "the manifest has no [[file]] entries".into(),
        );
    }

    let mut diags = ctx.diags;
    diags.sort_by_key(|d| (d.line, d.col));
    if diags.iter().any(|d| d.severity == Severity::Error) {
        return Err(diags);
    }
    let (Some(name), output_name) = (raw.name, raw.output_name) else {
        return Err(diags);
    };
    let manifest = Manifest::new(
        name.into_inner(),
        output_name.map(Spanned::into_inner),
        defaults.dir_mode,
        defaults.owner,
        entries,
    );
    Ok((manifest, diags))
}
