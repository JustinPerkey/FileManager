//! Decision logic of the task-plan guard: a one-for-one port of the former
//! `task-plan-only.mjs`. Pure: nothing is read from disk.
//!
//! Implementers may read only `docs/plans/tasks/`. Everything else under
//! `docs/plans/` is blocked for Read/Edit/Write/NotebookEdit; Glob, Grep and
//! Bash are blocked when they name the project-plans marker, and Grep also when
//! its search root would sweep `docs/plans/` without excluding markdown.

use std::path::Path;

use serde_json::Value;

const PLANS: &str = "docs/plans";
const ALLOWED: &str = "docs/plans/tasks";
const FORBIDDEN_MARKER: &str = "plans/project";

/// The outcome of one tool call.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    /// Block, with the full text to print on stderr.
    Block(String),
}

impl Decision {
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Decision::Allow => 0,
            Decision::Block(_) => 2,
        }
    }
}

/// Decide on one hook invocation, using the host platform's path rules.
#[must_use]
pub fn decide(input: &str, env_project_dir: Option<&str>, process_cwd: &Path) -> Decision {
    decide_with(input, env_project_dir, process_cwd, cfg!(windows))
}

/// [`decide`] with the Windows behaviour (case-insensitive, `win32` path
/// resolution) chosen explicitly, so tests cover both on every platform.
#[must_use]
pub fn decide_with(
    input: &str,
    env_project_dir: Option<&str>,
    process_cwd: &Path,
    win: bool,
) -> Decision {
    let trimmed = input.trim();
    let Ok(parsed) = serde_json::from_str::<Value>(if trimmed.is_empty() { "{}" } else { trimmed })
    else {
        return Decision::Allow; // unreadable hook input: a guardrail fails open
    };
    let Value::Object(obj) = &parsed else {
        return Decision::Allow; // the script would throw (exit 1): not a block
    };
    let process_cwd = process_cwd.to_string_lossy();
    let tool = match obj.get("tool_name") {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };
    let empty = Value::Null;
    let ti = obj
        .get("tool_input")
        .filter(|v| !v.is_null())
        .unwrap_or(&empty);
    let input_cwd = obj.get("cwd").filter(|v| truthy(v)).map(js_string);
    let root_src = env_project_dir
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .or_else(|| input_cwd.clone())
        .unwrap_or_else(|| process_cwd.to_string());
    let root = normalize(&root_src, win);
    let cwd = normalize(&input_cwd.unwrap_or_else(|| root.clone()), win);
    let ctx = Ctx {
        root,
        cwd,
        process_cwd: process_cwd.to_string(),
        win,
    };

    match check(&ctx, tool, ti) {
        Some(reason) => Decision::Block(format!(
            "Blocked: {reason}\nImplementers read only the task plan named in their prompt \
             (under {ALLOWED}/). If that plan is missing something you need, stop \
             and report the gap instead of looking for it in a project plan.\n"
        )),
        None => Decision::Allow,
    }
}

/// Normalised project root and working directory, plus the platform flag.
pub struct Ctx {
    pub root: String,
    pub cwd: String,
    pub process_cwd: String,
    pub win: bool,
}

/// `ti.<key>`, treating null and absent alike (JS `undefined`/`??`).
fn field<'a>(ti: &'a Value, key: &str) -> Option<&'a Value> {
    ti.get(key).filter(|v| !v.is_null())
}

fn check(ctx: &Ctx, tool: &str, ti: &Value) -> Option<String> {
    match tool {
        "Read" | "Edit" | "Write" | "NotebookEdit" => {
            let p = field(ti, "file_path").or_else(|| field(ti, "notebook_path"))?;
            if truthy(p) && is_forbidden_file(ctx, &js_string(p)) {
                Some(format!("{tool} of {}", js_string(p)))
            } else {
                None
            }
        }
        "Glob" => {
            if mentions_forbidden(field(ti, "pattern"), ctx.win)
                || mentions_forbidden(field(ti, "path"), ctx.win)
            {
                Some(format!("Glob naming {FORBIDDEN_MARKER}"))
            } else {
                None
            }
        }
        "Grep" => {
            if mentions_forbidden(field(ti, "path"), ctx.win)
                || mentions_forbidden(field(ti, "glob"), ctx.win)
            {
                return Some(format!("Grep naming {FORBIDDEN_MARKER}"));
            }
            let search = field(ti, "path").map_or_else(|| ctx.cwd.clone(), js_string);
            let search_root = rel(ctx, &search)?;
            if covers_plans(&search_root) && !excludes_markdown(ti) {
                let shown = if search_root.is_empty() {
                    "."
                } else {
                    &search_root
                };
                Some(format!(
                    "Grep over \"{shown}\" would search {PLANS}/. \
                     Scope it to a source directory (crates/, apps/, ...), or set \
                     type (e.g. \"rust\", \"ts\") or a non-markdown glob"
                ))
            } else {
                None
            }
        }
        "Bash" => mentions_forbidden(field(ti, "command"), ctx.win)
            .then(|| format!("Bash command naming {FORBIDDEN_MARKER}")),
        _ => None,
    }
}

/// JS truthiness of a JSON value.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// JS `String(v)` for a JSON value.
fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(|x| {
                if x.is_null() {
                    String::new()
                } else {
                    js_string(x)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
        other => other.to_string(),
    }
}

/// A file is forbidden when it sits under `docs/plans/` but not under
/// `docs/plans/tasks/`.
#[must_use]
pub fn is_forbidden_file(ctx: &Ctx, p: &str) -> bool {
    match rel(ctx, p) {
        None => false,
        Some(r) => within(&r, PLANS) && !within(&r, ALLOWED),
    }
}

/// True when a search rooted at `r` would descend into `docs/plans/` beyond
/// `tasks/`.
#[must_use]
pub fn covers_plans(r: &str) -> bool {
    r.is_empty() || within(PLANS, r) || (within(r, PLANS) && !within(r, ALLOWED))
}

/// True when a Grep's `type` or `glob` restricts it to non-markdown files.
#[must_use]
pub fn excludes_markdown(ti: &Value) -> bool {
    let is_md = |ext: &str| {
        let l = ext.to_lowercase();
        l == "md" || l == "markdown"
    };
    if let Some(t) = field(ti, "type").filter(|v| truthy(v)) {
        return !is_md(&js_string(t));
    }
    if let Some(g) = field(ti, "glob").filter(|v| truthy(v)) {
        let exts: Vec<String> = glob_extensions(&js_string(g));
        return !exts.is_empty() && !exts.iter().any(|e| is_md(e));
    }
    false
}

/// Every `\.\{?([a-z0-9,]+)\}?` match (case-insensitive), split on commas.
fn glob_extensions(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let in_class = |c: char| c.is_ascii_alphanumeric() || c == ',';
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '.' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        if j < chars.len() && chars[j] == '{' {
            j += 1;
        }
        let start = j;
        while j < chars.len() && in_class(chars[j]) {
            j += 1;
        }
        if j == start {
            i += 1; // `{` (if any) cannot start the class either: no match here
            continue;
        }
        let group: String = chars[start..j].iter().collect();
        out.extend(group.split(',').map(str::to_owned));
        if j < chars.len() && chars[j] == '}' {
            j += 1;
        }
        i = j;
    }
    out
}

/// True when `v` is a string whose normalised form contains the marker.
#[must_use]
pub fn mentions_forbidden(v: Option<&Value>, win: bool) -> bool {
    matches!(v, Some(Value::String(s)) if normalize(s, win).contains(FORBIDDEN_MARKER))
}

/// Repo-relative, forward-slashed path; `None` when outside the repository.
#[must_use]
pub fn rel(ctx: &Ctx, p: &str) -> Option<String> {
    let resolved = if ctx.win {
        resolve_win32(&ctx.cwd, p, &ctx.process_cwd)
    } else {
        resolve_posix(&[&ctx.cwd, p], &ctx.process_cwd)
    };
    let abs = normalize(&resolved, ctx.win);
    let r = relative_posix(&ctx.root, &abs, &ctx.process_cwd);
    if r.starts_with("..") || r.starts_with('/') {
        return None;
    }
    Some(r)
}

#[must_use]
pub fn within(child: &str, parent: &str) -> bool {
    child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Forward slashes, no trailing slash; lower-cased on Windows, whose paths are
/// case-insensitive.
#[must_use]
pub fn normalize(p: &str, win: bool) -> String {
    let s = p.replace('\\', "/");
    let s = s.trim_end_matches('/');
    if win {
        s.to_lowercase()
    } else {
        s.to_owned()
    }
}

/// Collapse `.` and `..` segments (Node's `normalizeString`).
fn collapse(segments: impl Iterator<Item = String>, allow_above_root: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for seg in segments {
        match seg.as_str() {
            "" | "." => {}
            ".." => {
                if out.last().is_some_and(|l| l != "..") {
                    out.pop();
                } else if allow_above_root {
                    out.push("..".into());
                }
            }
            _ => out.push(seg),
        }
    }
    out
}

/// Node `path.posix.resolve(...args)` (empty arguments skipped).
fn resolve_posix(args: &[&str], process_cwd: &str) -> String {
    let mut tail = String::new();
    let mut absolute = false;
    for arg in args
        .iter()
        .rev()
        .copied()
        .chain(std::iter::once(process_cwd))
    {
        if arg.is_empty() {
            continue;
        }
        tail = format!("{arg}/{tail}");
        if arg.starts_with('/') {
            absolute = true;
            break;
        }
    }
    let segs = collapse(tail.split('/').map(str::to_owned), !absolute);
    let joined = segs.join("/");
    if absolute {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".into()
    } else {
        joined
    }
}

/// Node `path.posix.relative(from, to)`.
fn relative_posix(from: &str, to: &str, process_cwd: &str) -> String {
    let from = resolve_posix(&[from], process_cwd);
    let to = resolve_posix(&[to], process_cwd);
    if from == to {
        return String::new();
    }
    let f: Vec<&str> = from.split('/').filter(|s| !s.is_empty()).collect();
    let t: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let common = f.iter().zip(&t).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; f.len() - common];
    parts.extend(&t[common..]);
    parts.join("/")
}

fn is_sep(c: char) -> bool {
    c == '/' || c == '\\'
}

/// Node `win32.resolve` root parsing: (root end, device, is absolute).
fn win_root(p: &str) -> (usize, String, bool) {
    let c: Vec<char> = p.chars().collect();
    let len = c.len();
    let slice = |a: usize, b: usize| -> String { c[a..b].iter().collect() };
    if len == 0 {
        return (0, String::new(), false);
    }
    let mut root_end = 0;
    let mut device = String::new();
    let mut absolute = false;
    if is_sep(c[0]) {
        absolute = true;
        if len > 1 && is_sep(c[1]) {
            let mut j = 2;
            let mut last = j;
            while j < len && !is_sep(c[j]) {
                j += 1;
            }
            if j < len && j != last {
                let first = slice(last, j);
                last = j;
                while j < len && is_sep(c[j]) {
                    j += 1;
                }
                if j < len && j != last {
                    last = j;
                    while j < len && !is_sep(c[j]) {
                        j += 1;
                    }
                    if j == len {
                        device = format!("\\\\{first}\\{}", slice(last, len));
                        root_end = j;
                    } else if j != last {
                        device = format!("\\\\{first}\\{}", slice(last, j));
                        root_end = j;
                    }
                }
            }
        } else {
            root_end = 1;
        }
    } else if len > 1 && c[0].is_ascii_alphabetic() && c[1] == ':' {
        device = slice(0, 2);
        root_end = 2;
        if len > 2 && is_sep(c[2]) {
            absolute = true;
            root_end = 3;
        }
    }
    (root_end, device, absolute)
}

/// Node `path.win32.resolve(cwd, p)` (without the per-drive environment
/// variable lookup, which only matters for a bare drive-relative path).
fn resolve_win32(cwd: &str, p: &str, process_cwd: &str) -> String {
    let mut resolved_device = String::new();
    let mut resolved_tail = String::new();
    let mut resolved_absolute = false;
    let args = [cwd, p];
    for i in (-1_i32..2).rev() {
        let path: String = if i >= 0 {
            let a = args[usize::try_from(i).unwrap_or(0)];
            if a.is_empty() {
                continue;
            }
            a.to_owned()
        } else if resolved_device.is_empty() {
            process_cwd.to_owned()
        } else {
            let pc: Vec<char> = process_cwd.chars().collect();
            let same_drive = pc.len() >= 2
                && pc[..2].iter().collect::<String>().to_lowercase()
                    == resolved_device.to_lowercase();
            if !same_drive && pc.get(2) == Some(&'\\') {
                format!("{resolved_device}\\")
            } else {
                process_cwd.to_owned()
            }
        };
        let (root_end, device, absolute) = win_root(&path);
        if !device.is_empty()
            && !resolved_device.is_empty()
            && device.to_lowercase() != resolved_device.to_lowercase()
        {
            continue;
        }
        if resolved_device.is_empty() && !device.is_empty() {
            resolved_device = device;
        }
        if !resolved_absolute {
            let rest: String = path.chars().skip(root_end).collect();
            resolved_tail = format!("{rest}\\{resolved_tail}");
            resolved_absolute = absolute;
        }
        if resolved_absolute && !resolved_device.is_empty() {
            break;
        }
    }
    let segs = collapse(
        resolved_tail.split(['\\', '/']).map(str::to_owned),
        !resolved_absolute,
    );
    let tail = segs.join("\\");
    if resolved_absolute {
        format!("{resolved_device}\\{tail}")
    } else if !(resolved_device.clone() + &tail).is_empty() {
        format!("{resolved_device}{tail}")
    } else {
        ".".into()
    }
}
