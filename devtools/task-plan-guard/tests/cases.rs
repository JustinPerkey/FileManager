//! Table-driven cases for the guard. Nothing here touches the disk.
//!
//! Setting `PARITY_DUMP` (any value) makes `dump_parity_cases` write the
//! non-Windows rows as JSON lines to `parity-cases.jsonl` in
//! `CARGO_TARGET_TMPDIR`, for comparison with the former Node script.

use std::path::Path;

use task_plan_guard::{append_trace, decide_with, trace_line, Decision};

struct Case {
    name: &'static str,
    tool: &'static str,
    input: &'static str,
    cwd: Option<&'static str>,
    project_dir: Option<&'static str>,
    win: bool,
    /// `None` allows; `Some(reason)` blocks with `Blocked: <reason>`.
    block: Option<&'static str>,
}

const fn c(
    name: &'static str,
    tool: &'static str,
    input: &'static str,
    block: Option<&'static str>,
) -> Case {
    Case {
        name,
        tool,
        input,
        cwd: Some("/repo"),
        project_dir: Some("/repo"),
        win: false,
        block,
    }
}

const fn w(
    name: &'static str,
    tool: &'static str,
    input: &'static str,
    cwd: &'static str,
    project_dir: &'static str,
    block: Option<&'static str>,
) -> Case {
    Case {
        name,
        tool,
        input,
        cwd: Some(cwd),
        project_dir: Some(project_dir),
        win: true,
        block,
    }
}

const GREP_HINT: fn(&str) -> String = |root| {
    format!(
        "Grep over \"{root}\" would search docs/plans/. Scope it to a source directory \
         (crates/, apps/, ...), or set type (e.g. \"rust\", \"ts\") or a non-markdown glob"
    )
};

#[expect(clippy::too_many_lines, reason = "a data table; one row per case")]
fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    for tool in ["Read", "Edit", "Write"] {
        v.push(c(
            "project plan, relative",
            tool,
            r#"{"file_path":"docs/plans/project/x.md"}"#,
            Some(match tool {
                "Read" => "Read of docs/plans/project/x.md",
                "Edit" => "Edit of docs/plans/project/x.md",
                _ => "Write of docs/plans/project/x.md",
            }),
        ));
    }
    v.extend([
        c(
            "project plan, absolute",
            "Read",
            r#"{"file_path":"/repo/docs/plans/project/x.md"}"#,
            Some("Read of /repo/docs/plans/project/x.md"),
        ),
        c(
            "notebook_path",
            "NotebookEdit",
            r#"{"notebook_path":"docs/plans/project/n.ipynb"}"#,
            Some("NotebookEdit of docs/plans/project/n.ipynb"),
        ),
        c(
            "task plan",
            "Read",
            r#"{"file_path":"docs/plans/tasks/a/M1.md"}"#,
            None,
        ),
        c(
            "task plan, absolute",
            "Write",
            r#"{"file_path":"/repo/docs/plans/tasks/a/M1.md"}"#,
            None,
        ),
        c(
            "outside the repo",
            "Read",
            r#"{"file_path":"/etc/passwd"}"#,
            None,
        ),
        c(
            "sibling dir sharing the root prefix",
            "Read",
            r#"{"file_path":"/repo2/docs/plans/project/x.md"}"#,
            None,
        ),
        c(
            "plans README",
            "Read",
            r#"{"file_path":"docs/plans/README.md"}"#,
            Some("Read of docs/plans/README.md"),
        ),
        c(
            "dotdot collapses into project",
            "Read",
            r#"{"file_path":"docs/plans/tasks/../project/x.md"}"#,
            Some("Read of docs/plans/tasks/../project/x.md"),
        ),
        c(
            "dotdot leaves the repo",
            "Read",
            r#"{"file_path":"../outside/x.md"}"#,
            None,
        ),
        c(
            "source file",
            "Read",
            r#"{"file_path":"crates/fm-core/src/lib.rs"}"#,
            None,
        ),
        c("no path", "Read", r"{}", None),
        c("empty path", "Read", r#"{"file_path":""}"#, None),
        c(
            "relative to a subdirectory cwd",
            "Read",
            r#"{"file_path":"../docs/plans/project/x.md"}"#,
            None,
        ),
        // Backslashes are not separators for the POSIX resolve, but normalize
        // turns them into slashes afterwards: no dotdot collapsing.
        c(
            "backslash path on posix",
            "Read",
            r#"{"file_path":"docs\\plans\\project\\x.md"}"#,
            Some("Read of docs\\plans\\project\\x.md"),
        ),
        c(
            "mixed case on posix",
            "Read",
            r#"{"file_path":"DOCS/Plans/Project/x.md"}"#,
            None,
        ),
        w(
            "windows: mixed case and backslashes",
            "Read",
            r#"{"file_path":"DOCS\\Plans\\Project\\x.md"}"#,
            "C:\\Repo",
            "C:\\Repo",
            Some("Read of DOCS\\Plans\\Project\\x.md"),
        ),
        w(
            "windows: absolute with drive",
            "Read",
            r#"{"file_path":"C:\\Repo\\docs\\plans\\project\\x.md"}"#,
            "C:\\Repo",
            "C:\\Repo",
            Some("Read of C:\\Repo\\docs\\plans\\project\\x.md"),
        ),
        w(
            "windows: other drive",
            "Read",
            r#"{"file_path":"D:\\Repo\\docs\\plans\\project\\x.md"}"#,
            "C:\\Repo",
            "C:\\Repo",
            None,
        ),
        w(
            "windows: leading backslash means the current drive",
            "Read",
            r#"{"file_path":"\\Repo\\docs\\plans\\project\\x.md"}"#,
            "C:\\Repo",
            "C:\\Repo",
            Some("Read of \\Repo\\docs\\plans\\project\\x.md"),
        ),
        w(
            "windows: unc path",
            "Read",
            r#"{"file_path":"\\\\srv\\share\\docs\\plans\\project\\x.md"}"#,
            "C:\\Repo",
            "C:\\Repo",
            None,
        ),
        w(
            "windows: unc root",
            "Read",
            r#"{"file_path":"\\\\srv\\share\\docs\\plans\\project\\x.md"}"#,
            "\\\\srv\\share",
            "\\\\srv\\share",
            Some("Read of \\\\srv\\share\\docs\\plans\\project\\x.md"),
        ),
        w(
            "windows: dotdot collapses with backslashes",
            "Read",
            r#"{"file_path":"docs\\plans\\tasks\\..\\project\\x.md"}"#,
            "C:\\Repo",
            "C:\\Repo",
            Some("Read of docs\\plans\\tasks\\..\\project\\x.md"),
        ),
        w(
            "windows: task plan",
            "Read",
            r#"{"file_path":"Docs\\Plans\\Tasks\\a\\M1.md"}"#,
            "C:\\Repo",
            "C:\\Repo",
            None,
        ),
        w(
            "windows: grep root with backslash path",
            "Grep",
            r#"{"path":"C:\\Repo\\Docs"}"#,
            "C:\\Repo",
            "C:\\Repo",
            Some("GREP:docs"),
        ),
        // Glob
        c(
            "glob marker in pattern",
            "Glob",
            r#"{"pattern":"docs/plans/project/**"}"#,
            Some("Glob naming plans/project"),
        ),
        c(
            "glob marker in path",
            "Glob",
            r#"{"pattern":"*.md","path":"docs/plans/project"}"#,
            Some("Glob naming plans/project"),
        ),
        c(
            "glob without marker",
            "Glob",
            r#"{"pattern":"crates/**/*.rs"}"#,
            None,
        ),
        c(
            "glob, plans tasks",
            "Glob",
            r#"{"pattern":"docs/plans/tasks/**"}"#,
            None,
        ),
        // Grep
        c(
            "grep path names marker",
            "Grep",
            r#"{"pattern":"x","path":"docs/plans/project"}"#,
            Some("Grep naming plans/project"),
        ),
        c(
            "grep glob names marker",
            "Grep",
            r#"{"pattern":"x","glob":"plans/project/*.md"}"#,
            Some("Grep naming plans/project"),
        ),
        c(
            "grep repo root, no type",
            "Grep",
            r#"{"pattern":"x"}"#,
            Some("GREP:."),
        ),
        c(
            "grep type rust",
            "Grep",
            r#"{"pattern":"x","type":"rust"}"#,
            None,
        ),
        c(
            "grep type md",
            "Grep",
            r#"{"pattern":"x","type":"md"}"#,
            Some("GREP:."),
        ),
        c(
            "grep type markdown",
            "Grep",
            r#"{"pattern":"x","type":"Markdown"}"#,
            Some("GREP:."),
        ),
        c(
            "grep glob rs",
            "Grep",
            r#"{"pattern":"x","glob":"*.rs"}"#,
            None,
        ),
        c(
            "grep glob md,rs",
            "Grep",
            r#"{"pattern":"x","glob":"*.{md,rs}"}"#,
            Some("GREP:."),
        ),
        c(
            "grep glob upper MD",
            "Grep",
            r#"{"pattern":"x","glob":"*.MD"}"#,
            Some("GREP:."),
        ),
        c(
            "grep glob no extension",
            "Grep",
            r#"{"pattern":"x","glob":"**/*"}"#,
            Some("GREP:."),
        ),
        c(
            "grep glob {rs,ts}",
            "Grep",
            r#"{"pattern":"x","glob":"*.{rs,ts}"}"#,
            None,
        ),
        c(
            "grep glob empty",
            "Grep",
            r#"{"pattern":"x","glob":""}"#,
            Some("GREP:."),
        ),
        c(
            "grep type beats glob",
            "Grep",
            r#"{"pattern":"x","type":"rust","glob":"*.md"}"#,
            None,
        ),
        c(
            "grep path crates",
            "Grep",
            r#"{"pattern":"x","path":"crates"}"#,
            None,
        ),
        c(
            "grep path docs",
            "Grep",
            r#"{"pattern":"x","path":"docs"}"#,
            Some("GREP:docs"),
        ),
        c(
            "grep path docs/plans",
            "Grep",
            r#"{"pattern":"x","path":"docs/plans"}"#,
            Some("GREP:docs/plans"),
        ),
        c(
            "grep path tasks",
            "Grep",
            r#"{"pattern":"x","path":"docs/plans/tasks"}"#,
            None,
        ),
        c(
            "grep absolute root",
            "Grep",
            r#"{"pattern":"x","path":"/repo"}"#,
            Some("GREP:."),
        ),
        c(
            "grep trailing slash root",
            "Grep",
            r#"{"pattern":"x","path":"/repo/"}"#,
            Some("GREP:."),
        ),
        c(
            "grep outside the repo",
            "Grep",
            r#"{"pattern":"x","path":"/usr"}"#,
            None,
        ),
        c(
            "grep null path is cwd",
            "Grep",
            r#"{"pattern":"x","path":null}"#,
            Some("GREP:."),
        ),
        // Bash
        c(
            "bash names marker",
            "Bash",
            r#"{"command":"cat docs/plans/project/x.md"}"#,
            Some("Bash command naming plans/project"),
        ),
        c(
            "bash marker with backslashes",
            "Bash",
            r#"{"command":"type docs\\plans\\project\\x.md"}"#,
            Some("Bash command naming plans/project"),
        ),
        c(
            "bash without marker",
            "Bash",
            r#"{"command":"cargo test"}"#,
            None,
        ),
        c(
            "bash tasks",
            "Bash",
            r#"{"command":"cat docs/plans/tasks/a/M1.md"}"#,
            None,
        ),
        c(
            "lone surrogate escape still judged",
            "Bash",
            r#"{"command":"cat docs/plans/project/x.md \ud800"}"#,
            Some("Bash command naming plans/project"),
        ),
        c(
            "lone low surrogate escape still judged",
            "Bash",
            r#"{"command":"\udc00 docs/plans/project/x.md"}"#,
            Some("Bash command naming plans/project"),
        ),
        c(
            "valid surrogate pair kept",
            "Read",
            r#"{"file_path":"docs/plans/project/\ud83d\ude00.md"}"#,
            Some("Read of docs/plans/project/\u{1F600}.md"),
        ),
        c(
            "escaped backslash before uD800 stays literal",
            "Bash",
            r#"{"command":"echo \\ud800 cargo test"}"#,
            None,
        ),
        c(
            "unknown tool",
            "WebFetch",
            r#"{"url":"docs/plans/project/x.md"}"#,
            None,
        ),
    ]);
    // Root from cwd when CLAUDE_PROJECT_DIR is absent.
    v.push(Case {
        name: "root from cwd",
        tool: "Read",
        input: r#"{"file_path":"docs/plans/project/x.md"}"#,
        cwd: Some("/other"),
        project_dir: None,
        win: false,
        block: Some("Read of docs/plans/project/x.md"),
    });
    v.push(Case {
        name: "project dir wins over cwd",
        tool: "Read",
        input: r#"{"file_path":"docs/plans/project/x.md"}"#,
        cwd: Some("/other"),
        project_dir: Some("/repo"),
        win: false,
        block: None,
    });
    v.push(Case {
        name: "absolute path under a project dir while cwd is a subdirectory",
        tool: "Read",
        input: r#"{"file_path":"/repo/docs/plans/project/x.md"}"#,
        cwd: Some("/repo/crates"),
        project_dir: Some("/repo"),
        win: false,
        block: Some("Read of /repo/docs/plans/project/x.md"),
    });
    v.push(Case {
        name: "grep from a subdirectory cwd",
        tool: "Grep",
        input: r#"{"pattern":"x"}"#,
        cwd: Some("/repo/crates"),
        project_dir: Some("/repo"),
        win: false,
        block: None,
    });
    v.push(Case {
        name: "grep from a parent of docs/plans",
        tool: "Grep",
        input: r#"{"pattern":"x"}"#,
        cwd: Some("/repo/docs"),
        project_dir: Some("/repo"),
        win: false,
        block: Some("GREP:docs"),
    });
    v
}

fn stdin_for(case: &Case) -> String {
    let mut s = format!(
        r#"{{"agent_type":"implementer","tool_name":"{}","tool_input":{}"#,
        case.tool, case.input
    );
    if let Some(cwd) = case.cwd {
        s.push_str(",\"cwd\":");
        s.push_str(&serde_json::to_string(cwd).unwrap());
    }
    s.push('}');
    s
}

fn expected_message(reason: &str) -> String {
    let reason = reason
        .strip_prefix("GREP:")
        .map_or_else(|| reason.to_owned(), GREP_HINT);
    format!(
        "Blocked: {reason}\nImplementers read only the task plan named in their prompt \
         (under docs/plans/tasks/). If that plan is missing something you need, stop \
         and report the gap instead of looking for it in a project plan.\n"
    )
}

#[test]
fn table() {
    for case in cases() {
        let got = decide_with(
            &stdin_for(&case),
            case.project_dir,
            Path::new("/proc-cwd"),
            case.win,
        );
        let want = match case.block {
            None => Decision::Allow,
            Some(reason) => Decision::Block(expected_message(reason)),
        };
        assert_eq!(got, want, "case: {} ({})", case.name, stdin_for(&case));
        assert_eq!(got.exit_code(), if case.block.is_some() { 2 } else { 0 });
    }
}

#[test]
fn unreadable_input_fails_open() {
    for input in ["", "   \n", "not json", "{", "[]", "null", "42", "\"Read\""] {
        let got = decide_with(input, Some("/repo"), Path::new("/repo"), false);
        assert_eq!(got, Decision::Allow, "input: {input:?}");
    }
}

#[test]
fn empty_object_and_missing_fields_allow() {
    for input in [
        "{}",
        r#"{"tool_name":"Read"}"#,
        r#"{"tool_name":"Bash","tool_input":null}"#,
    ] {
        assert_eq!(
            decide_with(input, Some("/repo"), Path::new("/repo"), false),
            Decision::Allow
        );
    }
}

#[test]
fn root_falls_back_to_process_cwd() {
    let input = r#"{"agent_type":"implementer","tool_name":"Read","tool_input":{"file_path":"docs/plans/project/x.md"}}"#;
    let got = decide_with(input, None, Path::new("/proc-root"), false);
    assert!(matches!(got, Decision::Block(_)));
    let got = decide_with(input, Some(""), Path::new("/proc-root"), false);
    assert!(matches!(got, Decision::Block(_)));
}

#[test]
fn glob_extension_parsing() {
    use serde_json::json;
    use task_plan_guard::excludes_markdown;
    assert!(excludes_markdown(&json!({"glob": "*.rs"})));
    assert!(!excludes_markdown(&json!({"glob": "*.{md,rs}"})));
    assert!(!excludes_markdown(&json!({"glob": "**/*"})));
    assert!(!excludes_markdown(&json!({"glob": "*.{}"})));
    assert!(excludes_markdown(&json!({"glob": "a.rs,b.ts"})));
    assert!(excludes_markdown(&json!({"type": "rust"})));
    assert!(!excludes_markdown(&json!({"type": "MD"})));
    assert!(!excludes_markdown(&json!({})));
}

#[test]
fn dump_parity_cases() {
    if std::env::var_os("PARITY_DUMP").is_none() {
        return;
    }
    let mut out = String::new();
    for case in cases().iter().filter(|c| !c.win) {
        let line = serde_json::json!({
            "name": case.name,
            "stdin": stdin_for(case),
            "project_dir": case.project_dir,
        });
        out.push_str(&line.to_string());
        out.push('\n');
    }
    let file = Path::new(env!("CARGO_TARGET_TMPDIR")).join("parity-cases.jsonl");
    std::fs::write(file, out).unwrap();
}

fn read_project_plan(agent: &str) -> String {
    format!(
        r#"{{{agent}"tool_name":"Read","tool_input":{{"file_path":"docs/plans/project/x.md"}},"cwd":"/repo"}}"#
    )
}

fn judge(input: &str) -> Decision {
    decide_with(input, Some("/repo"), Path::new("/repo"), false)
}

#[test]
fn guarded_roles_are_blocked() {
    for role in ["implementer", "ui-implementer"] {
        let got = judge(&read_project_plan(&format!(r#""agent_type":"{role}","#)));
        assert!(matches!(got, Decision::Block(_)), "role: {role}");
    }
}

#[test]
fn other_callers_are_allowed() {
    for agent in [
        String::new(),
        r#""agent_type":"planner","#.into(),
        r#""agent_type":"reviewer","#.into(),
        r#""agent_type":"Explore","#.into(),
        r#""agent_type":"general-purpose","#.into(),
        r#""agent_type":"some-plugin:implementer","#.into(),
        r#""agent_type":"Implementer","#.into(),
        r#""agent_type":"","#.into(),
        r#""agent_type":null,"#.into(),
        r#""agent_type":7,"#.into(),
        r#""agent_type":["implementer"],"#.into(),
        r#""agent_type":{"a":"implementer"},"#.into(),
    ] {
        assert_eq!(
            judge(&read_project_plan(&agent)),
            Decision::Allow,
            "{agent}"
        );
    }
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("tpg-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn trace_line_format() {
    let dir = temp_dir("fmt");
    let file = dir.join("trace.log");
    let blocked = r#"{"agent_type":"implementer","agent_id":"a1","tool_name":"Read","tool_input":{"file_path":"docs/plans/project/x.md"},"cwd":"/repo"}"#;
    let main_conv = r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#;
    for input in [blocked, main_conv, "not json"] {
        let line = trace_line(input, &judge(input));
        append_trace(&file, &line);
    }
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        text,
        "implementer\ta1\tRead\tblock\n-\t-\tBash\tallow\n-\t-\t-\tallow\n"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn trace_write_failure_is_ignored() {
    let dir = temp_dir("fail");
    // A directory cannot be opened for append as a file: the write fails.
    append_trace(&dir, "x");
    append_trace(&dir.join("missing").join("t.log"), "x");
    std::fs::remove_dir_all(&dir).unwrap();
}
