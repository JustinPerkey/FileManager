//! `PreToolUse` guard for the implementer and ui-implementer subagents.
//!
//! Implementers work from exactly one task plan under `docs/plans/tasks/`.
//! It runs from `.claude/settings.json` for every tool call, and guards only
//! calls whose `agent_type` is `implementer` or `ui-implementer`; everyone else
//! is allowed at once. If `TASK_PLAN_GUARD_TRACE` names a file, one line per call
//! is appended to it. This binary reads the tool call as JSON on stdin and blocks the obvious
//! routes to every other file under `docs/plans/` (see `lib.rs`). It is a
//! guardrail against accidental reads, not a sandbox.
//!
//! Exit 0 allows the call; exit 2 blocks it and feeds stderr back to the agent.
//!
//! The settings hook runs it with `cargo run --quiet --locked --manifest-path
//! ... --target-dir target/task-plan-guard`. The first call compiles it (it
//! depends only on `serde_json`); later calls start the cached binary. If
//! cargo cannot build or run it (a member's `Cargo.toml` mid-edit, a stale
//! lock), cargo exits with a code other than 2, so Claude Code reports a
//! non-blocking hook error and the call proceeds: the guard fails open, as it
//! does on unreadable input.

use std::io::Read;
use std::process::ExitCode;

use task_plan_guard::{append_trace, decide, trace_line, Decision};

fn main() -> ExitCode {
    let mut bytes = Vec::new();
    if std::io::stdin().read_to_end(&mut bytes).is_err() {
        return ExitCode::SUCCESS; // unreadable hook input: a guardrail fails open
    }
    let input = String::from_utf8_lossy(&bytes);
    let env_dir = std::env::var("CLAUDE_PROJECT_DIR").ok();
    let process_cwd = std::env::current_dir().unwrap_or_default();
    let decision = decide(&input, env_dir.as_deref(), &process_cwd);
    if let Some(path) = std::env::var_os("TASK_PLAN_GUARD_TRACE").filter(|p| !p.is_empty()) {
        append_trace(std::path::Path::new(&path), &trace_line(&input, &decision));
    }
    match decision {
        Decision::Allow => ExitCode::SUCCESS,
        Decision::Block(message) => {
            eprint!("{message}");
            ExitCode::from(2)
        }
    }
}
