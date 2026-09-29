//! Turns a raw manifest into a validated `Manifest`, collecting every error
//! and warning with its line and column.

use std::collections::{BTreeMap, HashMap, HashSet};

use toml::Spanned;

use super::mode::parse_mode;
use super::model::{Diagnostic, Entry, EntryFailure, Manifest, Owner, ParseReport, Severity};
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

/// Where a diagnostic came from, kept internally so the report can group it.
#[derive(Clone, Copy, Debug)]
pub(super) enum Origin {
    /// Outside every `[[file]]` table. `withholds` hides all entries.
    Header { withholds: bool },
    /// Inside the `[[file]]` table at this 1-based position.
    Table(usize),
}

/// Who a diagnostic is attributed to. The prefix rule lives here only.
#[derive(Clone, Copy)]
pub(super) enum Scope<'a> {
    /// Top level, no prefix.
    Top { withholds: bool },
    /// `[defaults]` unknown-key, serde and not-a-table errors, prefixed
    /// ``[defaults]: ``; withholds. Bad values in `[defaults]` are reported
    /// through `Top { withholds: true }` instead, since their messages already
    /// start with `defaults.<field>`.
    Defaults,
    /// A `[[file]]` table: its non-empty string id, and its position.
    File(Option<&'a str>, usize),
}

impl<'a> Scope<'a> {
    /// A table's scope: an empty id counts as no id.
    pub(super) fn file(id: &'a str, n: usize) -> Scope<'a> {
        Scope::File((!id.is_empty()).then_some(id), n)
    }

    fn origin(self) -> Origin {
        match self {
            Scope::Top { withholds } => Origin::Header { withholds },
            Scope::Defaults => Origin::Header { withholds: true },
            Scope::File(_, n) => Origin::Table(n),
        }
    }
}

pub(super) struct Ctx<'a> {
    index: LineIndex<'a>,
    diags: Vec<(Origin, Diagnostic)>,
}

impl<'a> Ctx<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        Ctx {
            index: LineIndex::new(text),
            diags: Vec::new(),
        }
    }

    /// An error at byte offset `at`, attributed through `scope`.
    pub(super) fn error(&mut self, scope: Scope, at: usize, message: String) {
        self.push(Severity::Error, scope, at, message);
    }

    fn warn(&mut self, scope: Scope, at: usize, message: String) {
        self.push(Severity::Warning, scope, at, message);
    }

    fn push(&mut self, severity: Severity, scope: Scope, at: usize, message: String) {
        let (line, col) = self.index.locate(at);
        let (message, entry_id) = match scope {
            Scope::Top { .. } => (message, None),
            Scope::Defaults => (format!("[defaults]: {message}"), None),
            Scope::File(Some(id), _) => (format!("file `{id}`: {message}"), Some(id.to_string())),
            Scope::File(None, n) => (format!("[[file]] #{n}: {message}"), None),
        };
        self.diags.push((
            scope.origin(),
            Diagnostic {
                severity,
                line,
                col,
                entry_id,
                message,
            },
        ));
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

fn to_u32(ctx: &mut Ctx, scope: Scope, field: &str, v: &Spanned<i64>) -> Option<u32> {
    match u32::try_from(*v.get_ref()) {
        Ok(n) => Some(n),
        Err(_) => {
            ctx.error(
                scope,
                v.span().start,
                format!("{field} `{}` does not fit in u32", v.get_ref()),
            );
            None
        }
    }
}

fn mode_field(ctx: &mut Ctx, scope: Scope, field: &str, v: &Spanned<String>) -> Option<u32> {
    match parse_mode(v.get_ref()) {
        Ok(m) => Some(m),
        Err(e) => {
            ctx.error(scope, v.span().start, format!("{field}: {e}"));
            None
        }
    }
}

fn owner_name(ctx: &mut Ctx, scope: Scope, field: &str, v: &Spanned<String>) -> Option<String> {
    let s = v.get_ref();
    if s.is_empty() || s.len() > MAX_OWNER_NAME {
        ctx.error(
            scope,
            v.span().start,
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

impl Defaults {
    fn builtin() -> Defaults {
        Defaults {
            mode: DEFAULT_MODE,
            dir_mode: DEFAULT_DIR_MODE,
            owner: Owner {
                uid: 0,
                gid: 0,
                uname: "root".into(),
                gname: "root".into(),
            },
        }
    }
}

fn resolve_defaults(ctx: &mut Ctx, raw: Option<&RawDefaults>) -> Defaults {
    let mut d = Defaults::builtin();
    let Some(raw) = raw else { return d };
    // Every `[defaults]` error withholds the entries; the field name is the
    // prefix, so no `[defaults]: ` is added.
    let sc = Scope::Top { withholds: true };
    if let Some(v) = &raw.mode {
        d.mode = mode_field(ctx, sc, "defaults.mode", v).unwrap_or(d.mode);
    }
    if let Some(v) = &raw.dir_mode {
        d.dir_mode = mode_field(ctx, sc, "defaults.dir_mode", v).unwrap_or(d.dir_mode);
    }
    if let Some(v) = &raw.uid {
        d.owner.uid = to_u32(ctx, sc, "defaults.uid", v).unwrap_or(d.owner.uid);
    }
    if let Some(v) = &raw.gid {
        d.owner.gid = to_u32(ctx, sc, "defaults.gid", v).unwrap_or(d.owner.gid);
    }
    if let Some(v) = &raw.uname {
        if let Some(s) = owner_name(ctx, sc, "defaults.uname", v) {
            d.owner.uname = s;
        }
    }
    if let Some(v) = &raw.gname {
        if let Some(s) = owner_name(ctx, sc, "defaults.gname", v) {
            d.owner.gname = s;
        }
    }
    d
}

fn resolve_entry(ctx: &mut Ctx, f: &RawFile, defaults: &Defaults, index: usize) -> Option<Entry> {
    let id_str = f.id.get_ref().as_str();
    let sc = Scope::file(id_str, index);
    let mut ok = true;

    if id_str.is_empty() {
        ctx.error(sc, f.id.span().start, "id must not be empty".into());
        ok = false;
    }

    let source = f.source.get_ref();
    if let Some(why) = bad_leaf(source) {
        ctx.error(sc, f.source.span().start, format!("source {why}"));
        ok = false;
    }

    let name = f
        .name
        .as_ref()
        .map_or(source.as_str(), |n| n.get_ref().as_str());
    if let Some(n) = &f.name {
        if let Some(why) = bad_leaf(name) {
            ctx.error(sc, n.span().start, format!("name {why}"));
            ok = false;
        }
    }

    let dir = f.dir.get_ref();
    if let Some(why) = bad_dir(dir) {
        ctx.error(sc, f.dir.span().start, format!("dir {why}"));
        ok = false;
    }

    let mut mode = defaults.mode;
    if let Some(v) = &f.mode {
        match mode_field(ctx, sc, "mode", v) {
            Some(m) => mode = m,
            None => ok = false,
        }
    }
    let mut owner = defaults.owner.clone();
    if let Some(v) = &f.uid {
        match to_u32(ctx, sc, "uid", v) {
            Some(n) => owner.uid = n,
            None => ok = false,
        }
    }
    if let Some(v) = &f.gid {
        match to_u32(ctx, sc, "gid", v) {
            Some(n) => owner.gid = n,
            None => ok = false,
        }
    }
    if let Some(v) = &f.uname {
        match owner_name(ctx, sc, "uname", v) {
            Some(s) => owner.uname = s,
            None => ok = false,
        }
    }
    if let Some(v) = &f.gname {
        match owner_name(ctx, sc, "gname", v) {
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

/// The report for a manifest that cannot be read at all: an empty incomplete
/// manifest and the one error that stopped it.
pub(super) fn withheld_report(error: Diagnostic) -> ParseReport {
    let d = Defaults::builtin();
    ParseReport {
        manifest: Manifest::new(String::new(), None, d.dir_mode, d.owner, Vec::new(), false),
        entries_withheld: true,
        errors: vec![error],
        failures: Vec::new(),
        warnings: Vec::new(),
    }
}

/// A resolved entry with the raw table it came from.
struct Resolved<'r> {
    /// Index into `raw.files`.
    slot: usize,
    entry: Entry,
    file: &'r RawFile,
}

impl Resolved<'_> {
    /// Where target-path errors point: the `name` value, else `dir`.
    fn target_at(&self) -> usize {
        self.file
            .name
            .as_ref()
            .map_or_else(|| self.file.dir.span(), |n| n.span())
            .start
    }
    fn scope(&self, raw: &RawDoc) -> Scope<'_> {
        Scope::file(self.entry.id(), raw.files[self.slot].index)
    }
}

pub(super) fn validate(mut ctx: Ctx, raw: RawDoc) -> ParseReport {
    let hard = Scope::Top { withholds: true };
    let soft = Scope::Top { withholds: false };
    if let Some(v) = &raw.version {
        if *v.get_ref() != 1 {
            ctx.error(
                hard,
                v.span().start,
                format!(
                    "unsupported version {}; this program reads version 1",
                    v.get_ref()
                ),
            );
        }
    }
    if let Some(n) = &raw.name {
        if n.get_ref().is_empty() {
            ctx.error(soft, n.span().start, "name must not be empty".into());
        }
    }
    let mut output_name_ok = true;
    if let Some(o) = &raw.output_name {
        let s = o.get_ref();
        if s.is_empty() || s.contains(['/', '\\', '\0']) {
            output_name_ok = false;
            ctx.error(
                soft,
                o.span().start,
                "output_name must not be empty or contain `/`, `\\` or NUL".into(),
            );
        }
    }

    let defaults = resolve_defaults(&mut ctx, raw.defaults.as_ref());

    let mut resolved: Vec<Resolved> = Vec::new();
    for (i, slot) in raw.files.iter().enumerate() {
        let Some(file) = &slot.file else { continue };
        if let Some(entry) = resolve_entry(&mut ctx, file, &defaults, slot.index) {
            resolved.push(Resolved {
                slot: i,
                entry,
                file,
            });
        }
    }

    // A table that failed on its own takes no part in the cross-entry checks.
    // (Duplicate-id errors are pushed after this, so a table failing only
    // through a duplicate id still takes part.)
    let own_failed: HashSet<usize> = ctx
        .diags
        .iter()
        .filter_map(|(o, d)| match o {
            Origin::Table(n) if d.severity == Severity::Error => Some(*n),
            _ => None,
        })
        .collect();
    resolved.retain(|r| !own_failed.contains(&raw.files[r.slot].index));

    // Duplicate ids, from the raw id, so tables that failed to deserialize
    // take part. Every table sharing an id is reported.
    let mut by_id: HashMap<&str, Vec<&super::raw::RawFileSlot>> = HashMap::new();
    for slot in &raw.files {
        if let Some(i) = &slot.id {
            if !i.get_ref().is_empty() {
                by_id.entry(i.get_ref().as_str()).or_default().push(slot);
            }
        }
    }
    for (id, slots) in &by_id {
        if slots.len() < 2 {
            continue;
        }
        let line_of = |s: &&super::raw::RawFileSlot| {
            ctx.index
                .locate(s.id.as_ref().map_or(0, |i| i.span().start))
                .0
        };
        let first_line = line_of(&slots[0]);
        let later: Vec<u32> = slots[1..].iter().map(line_of).collect();
        for s in &slots[1..] {
            let at = s.id.as_ref().map_or(0, |i| i.span().start);
            ctx.error(
                Scope::file(id, s.index),
                at,
                format!("duplicate id (first used on line {first_line})"),
            );
        }
        let list: Vec<String> = later.iter().map(u32::to_string).collect();
        let msg = if list.len() == 1 {
            format!("duplicate id (also used on line {})", list[0])
        } else {
            format!("duplicate id (also used on lines {})", list.join(", "))
        };
        let at = slots[0].id.as_ref().map_or(0, |i| i.span().start);
        ctx.error(Scope::file(id, slots[0].index), at, msg);
    }

    // Duplicate targets, reported on both entries of each pair.
    let mut targets: HashMap<String, Vec<usize>> = HashMap::new();
    for (k, r) in resolved.iter().enumerate() {
        let path = r.entry.target_path();
        let prev = targets.entry(path.clone()).or_default();
        for &p in prev.iter() {
            let p = &resolved[p];
            ctx.error(
                r.scope(&raw),
                r.target_at(),
                format!(
                    "target path `{path}` is already used by file `{}`",
                    p.entry.id()
                ),
            );
            ctx.error(
                p.scope(&raw),
                p.target_at(),
                format!(
                    "target path `{path}` is also used by file `{}`",
                    r.entry.id()
                ),
            );
        }
        prev.push(k);
    }

    // Long names and shared sources are warnings.
    let mut sources: HashMap<String, &str> = HashMap::new();
    for r in &resolved {
        let path = r.entry.target_path();
        if path.len() >= LONG_NAME_BYTES {
            ctx.warn(
                r.scope(&raw),
                r.file.dir.span().start,
                format!(
                    "stored path `{path}` is 100 bytes or longer ({} bytes); \
                     the archive will use a GNU long-name record",
                    path.len()
                ),
            );
        }
        let key = r.entry.source().to_lowercase();
        if let Some(prev) = sources.get(&key) {
            ctx.warn(
                r.scope(&raw),
                r.file.source.span().start,
                format!(
                    "source `{}` matches file `{prev}` (compared case-insensitively); \
                     a dropped file cannot tell them apart, so pick their files per row",
                    r.entry.source()
                ),
            );
        } else {
            sources.insert(key, r.entry.id());
        }
    }

    // A file's stored path must not also be a directory another entry needs.
    for (i, a) in resolved.iter().enumerate() {
        let path = a.entry.target_path();
        for (j, b) in resolved.iter().enumerate() {
            if i != j && needed_dirs(b.entry.target_dir()).contains(&path.as_str()) {
                ctx.error(
                    a.scope(&raw),
                    a.target_at(),
                    format!(
                        "target path `{path}` is also a directory of file `{}`",
                        b.entry.id()
                    ),
                );
                ctx.error(
                    b.scope(&raw),
                    b.file.dir.span().start,
                    format!(
                        "dir needs `{path}`, which is the target path of file `{}`",
                        a.entry.id()
                    ),
                );
            }
        }
    }

    if raw.files.is_empty() {
        ctx.warn(soft, 0, "the manifest has no [[file]] entries".into());
    }

    // Group the diagnostics by where they came from.
    let Ctx { index, diags } = ctx;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut by_table: BTreeMap<usize, Vec<Diagnostic>> = BTreeMap::new();
    let mut entries_withheld = false;
    for (origin, d) in diags {
        if d.severity == Severity::Warning {
            warnings.push(d);
            continue;
        }
        match origin {
            Origin::Header { withholds } => {
                entries_withheld |= withholds;
                errors.push(d);
            }
            Origin::Table(n) => by_table.entry(n).or_default().push(d),
        }
    }
    let by_pos = |d: &Diagnostic| (d.line, d.col);
    errors.sort_by_key(by_pos);
    warnings.sort_by_key(by_pos);
    let failures: Vec<EntryFailure> = by_table
        .into_iter()
        .map(|(n, mut errs)| {
            errs.sort_by_key(by_pos);
            let slot = &raw.files[n - 1];
            EntryFailure {
                index: n as u32,
                id: slot
                    .id
                    .as_ref()
                    .map(|i| i.get_ref().clone())
                    .filter(|s| !s.is_empty()),
                source: slot.source.clone(),
                line: index.locate(slot.at).0,
                errors: errs,
            }
        })
        .collect();

    let failed: HashSet<usize> = failures.iter().map(|f| f.index as usize).collect();
    let entries: Vec<Entry> = if entries_withheld {
        Vec::new()
    } else {
        resolved
            .into_iter()
            .filter(|r| !failed.contains(&raw.files[r.slot].index))
            .map(|r| r.entry)
            .collect()
    };

    let valid = errors.is_empty() && failures.is_empty();
    let name = raw
        .name
        .map(Spanned::into_inner)
        .filter(|n| !n.is_empty())
        .unwrap_or_default();
    let output_name = raw
        .output_name
        .map(Spanned::into_inner)
        .filter(|_| output_name_ok);
    ParseReport {
        manifest: Manifest::new(
            name,
            output_name,
            defaults.dir_mode,
            defaults.owner,
            entries,
            valid,
        ),
        entries_withheld,
        errors,
        failures,
        warnings,
    }
}
