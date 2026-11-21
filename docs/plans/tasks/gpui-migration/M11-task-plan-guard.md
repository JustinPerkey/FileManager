# M11 — The task-plan guard hook as a Rust binary

Status: awaiting approval
Project: gpui-migration   Depends on: none (run it early: every later implementer run uses the hook)

## Goal

Replace `.claude/hooks/task-plan-only.mjs` (a Node script) with a small Rust
binary in the workspace, `devtools/task-plan-guard`, with **identical
behaviour**, and point the implementer and ui-implementer agents' hook at
it, so the repository needs no Node at all.

## Context

The `implementer` and `ui-implementer` subagents must read only the one
task plan they are given, under `docs/plans/tasks/`. A Claude Code
`PreToolUse` hook enforces this: `.claude/agents/implementer.md` and
`.claude/agents/ui-implementer.md` declare, in their front matter,

```yaml
hooks:
  PreToolUse:
    - matcher: "Read|Edit|Write|NotebookEdit|Grep|Glob|Bash"
      hooks:
        - type: command
          command: 'node "$CLAUDE_PROJECT_DIR/.claude/hooks/task-plan-only.mjs"'
```

The hook reads the tool call as JSON on stdin (`tool_name`, `tool_input`,
`cwd`), exits 0 to allow, or exits 2 with a message on stderr to block (the
message is fed back to the agent). Read `.claude/hooks/task-plan-only.mjs`
in full: it is the specification. In summary:

- `PLANS = "docs/plans"`, `ALLOWED = "docs/plans/tasks"`, marker
  `"plans/project"`.
- Unreadable or empty stdin → allow (fails open).
- Root = `CLAUDE_PROJECT_DIR`, else `input.cwd`, else the process cwd;
  cwd = `input.cwd`, else root; both normalised.
- `Read`/`Edit`/`Write`/`NotebookEdit`: block when `file_path` (or
  `notebook_path`) is under `docs/plans/` but not under `docs/plans/tasks/`.
- `Glob`: block when `pattern` or `path` contains the marker.
- `Grep`: block when `path` or `glob` contains the marker; otherwise, when
  the search root (`path`, else cwd) covers `docs/plans/` and the call does
  not exclude markdown (`type` other than `md`/`markdown`; or a `glob` whose
  extensions, parsed by `\.\{?([a-z0-9,]+)\}?` case-insensitively, are
  non-empty and contain no `md`/`markdown`), block with the scoping hint.
- `Bash`: block when `command` contains the marker.
- Any other tool: allow.
- Paths: resolved against cwd (`path.resolve`: absolute paths kept,
  relative joined, `.` and `..` collapsed lexically), backslashes to `/`,
  trailing slashes removed, lower-cased **on Windows only**;
  repo-relative or `None` when outside the root.
- The block message: `Blocked: <reason>` then the fixed explanation lines,
  verbatim.

The human decided (2026-11-21) to port this to "a small Rust binary in the
workspace", called from the agent settings, keeping the behaviour identical.
This milestone changes agent configuration under that approval.

### What to build

- `devtools/task-plan-guard/`: a workspace member (add `devtools/*` to the
  root `members`), package and binary `task-plan-guard`,
  `[lints] workspace = true`, the workspace `version`/`edition`/`license`,
  `publish = false`. One dependency: `serde_json` (at the version the lock
  already has). No other crate, no `fm-*` crate, no GPUI.
- `src/main.rs`: reads stdin, calls `decide(input: &str, env_project_dir: Option<&str>, process_cwd: &Path) -> Decision`,
  and exits 0 or prints the message to stderr and exits 2. `decide` and its
  helpers live in `src/lib.rs` (pure, testable): `normalize`, `resolve`,
  `rel`, `within`, `is_forbidden_file`, `covers_plans`, `excludes_markdown`,
  `mentions_forbidden`, mirroring the JS functions one for one. Windows
  lower-casing is `cfg!(windows)`; make it a parameter of the pure functions
  so both behaviours are tested on every platform.
- The agent hook command, in both `.claude/agents/implementer.md` and
  `.claude/agents/ui-implementer.md`:

  ```
  cargo run --quiet --locked --manifest-path "$CLAUDE_PROJECT_DIR/devtools/task-plan-guard/Cargo.toml" --target-dir "$CLAUDE_PROJECT_DIR/target/task-plan-guard"
  ```

  How it is available when Claude Code runs it: every contributor has the
  repository's Rust toolchain (`rust-toolchain.toml`). The first call
  compiles the guard (a few seconds; it depends only on `serde_json`); later
  calls start the cached binary. The separate `--target-dir` means it never
  waits on the build lock of a `cargo build` the agent is running. If cargo
  cannot build or run it (for example while a member's `Cargo.toml` is
  mid-edit or the lock is stale), cargo exits with a code other than 2, so
  Claude Code reports a non-blocking hook error and the call proceeds: the
  guard fails open, as the `.mjs` does on unreadable input. Document this in
  a doc comment at the top of `src/main.rs` and in the agent files' prose
  where they mention the hook.
- Delete `.claude/hooks/task-plan-only.mjs` (and the directory if empty).
- Update every reference: `CLAUDE.md` (Layout: `devtools/task-plan-guard/`
  replaces the `.claude/hooks/task-plan-only.mjs` line; Layout block lists
  `devtools/`), the two agent files' prose, and the `.codex/agents/{implementer,ui-implementer}.toml`
  parenthetical "(Under Claude a hook enforces …)" so it names the new
  guard. The `.codex` twins have no hooks; only the wording changes.
- `docs/plans/README.md` also names the hook, but the guard blocks
  implementers from every file under `docs/plans/` outside `tasks/`. Do not
  try to edit it; put the one-sentence replacement in the PR description for
  the planner, who applies it.
- **Working under the guard you are porting:** the current hook blocks any
  Bash command whose text contains the marker string. Keep test inputs that
  contain it in files (the Rust test table, a JSON-lines file for the parity
  script under the scratch or `target/` directory) and pass them by file
  name, never inline on a Bash command line.

### Rules that bind this task

- **Identical behaviour.** Same allow/block decision and same stderr text
  for every input the `.mjs` handles.
- Each `.claude` agent and its `.codex` twin stay the same role.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.
- Tests never touch real user files; the guard reads nothing from disk.

## Files

Create: `devtools/task-plan-guard/{Cargo.toml,src/main.rs,src/lib.rs,tests/cases.rs}`.

Edit: `Cargo.toml`, `Cargo.lock`, `.claude/agents/implementer.md`,
`.claude/agents/ui-implementer.md`, `.codex/agents/implementer.toml`,
`.codex/agents/ui-implementer.toml`, `CLAUDE.md`.

Delete: `.claude/hooks/task-plan-only.mjs`.

Do not touch: other agents, `docs/plans/project/`, application code.

## Acceptance criteria

- The two agent files' hook command is the `cargo run` line above; no file
  in the repository names `task-plan-only.mjs` except historical plans.
- **Parity:** before deleting the `.mjs`, run a parity script (kept out of
  the repo, in the PR description) that feeds every case of
  `tests/cases.rs` to both `node .claude/hooks/task-plan-only.mjs` and the
  Rust binary, with the same `CLAUDE_PROJECT_DIR`, and compares exit code and
  stderr. All equal; paste the summary in the PR. If `node` is not
  available, say so; the table-driven tests are then the proof.
- From a Claude Code session, an implementer subagent's `Read` of
  `docs/plans/project/gpui-migration.md` is blocked with the expected
  message, and a `Read` of a task plan is allowed (record the check in the
  PR).
- fmt, clippy pedantic and `cargo test --workspace` pass on both CI jobs
  (the guard is a member, so the workspace commands cover it).

## Tests proving completion

`devtools/task-plan-guard/tests/cases.rs`, table-driven, each row: tool
name, tool input JSON, cwd, project dir, Windows flag → expected exit code
and stderr reason. At least:

- Read/Edit/Write/NotebookEdit of a project plan (relative and absolute),
  of a task plan, of a file outside the repo, of `docs/plans/README.md`
  (blocked: under `docs/plans/` but not `tasks/`), of
  `docs/plans/tasks/../project/x.md` (blocked after `..` collapsing).
- Windows: `DOCS\Plans\Project\x.md` blocked with lower-casing; the same on
  non-Windows allowed only if the JS allows it (match the JS).
- Glob with the marker in `pattern` or in `path`; without it.
- Grep: `path` naming the marker; repo root with no type (blocked, hint);
  repo root with `type: "rust"` (allowed), `type: "md"` (blocked);
  `glob: "*.rs"` (allowed), `glob: "*.{md,rs}"` (blocked), `glob: "**/*"`
  (no extension: blocked); `path: "crates"` (allowed);
  `path: "docs"` (blocked); `path: "docs/plans/tasks"` (allowed).
- Bash naming `plans/project`; Bash not naming it.
- Unknown tool → allowed; empty stdin and invalid JSON → allowed.
- Root from `CLAUDE_PROJECT_DIR`, else `cwd`.

Command: `cargo test -p task-plan-guard`.

## Out of scope

- Any change to what the guard blocks (identical behaviour).
- Other agent-role changes (the docs and governance milestone).

## Risks

- First-call latency (compiling) on a fresh checkout; later calls are fast.
- Fail-open windows while the workspace cannot build; the agent
  instructions still carry the rule, as they do under Codex.
- `path.resolve` semantics on Windows (drive letters, UNC, a leading `\`
  meaning the current drive): match the JS, and cover each in a Windows-flag
  test row.
