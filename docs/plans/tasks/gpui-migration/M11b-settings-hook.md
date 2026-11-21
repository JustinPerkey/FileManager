# M11b — Run the task-plan guard from `.claude/settings.json`, for the two implementer roles only

Status: approved (2026-11-21)
Project: gpui-migration   Depends on: M11 (landed)

## Goal

Make the task-plan guard actually run for `implementer` and
`ui-implementer` subagents in every Claude Code runtime, including cloud and
`-p` sessions, by calling it from a `PreToolUse` hook in
`.claude/settings.json` that applies the guard only when the hook input says
the calling agent is one of those two roles. Then remove the agent
frontmatter hooks.

## Context

The guard is `devtools/task-plan-guard` (a workspace member under
`devtools/*`): `src/lib.rs` has `decide(input, env_project_dir, process_cwd) -> Decision`
and `decide_with(…, win)`; `src/main.rs` reads the hook JSON on stdin and
exits 0 (allow) or 2 with the block message on stderr; `tests/cases.rs` is
the table of cases. Today it is wired only through the frontmatter of
`.claude/agents/implementer.md` and `.claude/agents/ui-implementer.md`:

```yaml
hooks:
  PreToolUse:
    - matcher: "Read|Edit|Write|NotebookEdit|Grep|Glob|Bash"
      hooks:
        - type: command
          command: 'cargo run --quiet --locked --manifest-path "$CLAUDE_PROJECT_DIR/devtools/task-plan-guard/Cargo.toml" --target-dir "$CLAUDE_PROJECT_DIR/target/task-plan-guard"'
```

**Finding (2026-11-21).** Live tests in the cloud runtime showed that this
frontmatter hook never fires: implementer subagents read project plans with
no hook error, with both the old Node guard and the Rust one. Claude Code's
hook reference explains it (code.claude.com/docs/en/hooks, `sub-agents`,
`permissions`; read with Claude Code 2.1.330):

- Frontmatter hooks of a **project** subagent run only after the
  workspace-trust dialog is accepted for the folder; `claude -p`, SDK and
  cloud sessions never show that dialog, and Claude Code then skips the
  hooks silently (a debug-log line only).
- **Hooks in settings files run** in those sessions ("What runs before you
  trust a folder": used under `-p` and the SDK), and they "also run inside
  subagents. When a subagent calls a tool, tool events such as `PreToolUse`
  … fire the same configured hooks as in the main conversation, and the
  input carries the `agent_id` and `agent_type` common input fields that
  identify the subagent."
- For a custom subagent, `agent_type` is the `name` field of its
  frontmatter (here `implementer`, `ui-implementer`); for plugin subagents
  it is plugin-scoped (`plugin:name`). In the main conversation it is absent,
  unless the session was started with `--agent <name>` (then it is that
  name).

The human decided (2026-11-21) to move the guard to a settings-file hook,
on the condition that the input identifies the calling subagent reliably
enough to guard only those two roles. This milestone changes
`.claude/settings.json` and the agent files under that decision.

### What to build

1. **The guard decides who is calling.** In `src/lib.rs`, before any other
   check: read the top-level string field `agent_type` of the input. If it
   is absent, not a string, or not exactly one of
   `GUARDED_AGENTS = ["implementer", "ui-implementer"]`, return
   `Decision::Allow`. Otherwise decide exactly as today. Unreadable input
   still allows (fails open), as today. The block message is unchanged.
2. **Optional trace for the live check.** If the environment variable
   `TASK_PLAN_GUARD_TRACE` names a file, append one line per call:
   `<agent_type or "-">\t<agent_id or "-">\t<tool_name>\t<allow|block>`. No
   other output; never fail because of the trace (ignore write errors).
3. **`.claude/settings.json`.** Keep every existing key
   (`extraKnownMarketplaces`, `enabledPlugins`) and add:

   ```json
   "hooks": {
     "PreToolUse": [
       {
         "matcher": "Read|Edit|Write|NotebookEdit|Grep|Glob|Bash",
         "hooks": [
           {
             "type": "command",
             "command": "cargo run --quiet --locked --manifest-path \"$CLAUDE_PROJECT_DIR/devtools/task-plan-guard/Cargo.toml\" --target-dir \"$CLAUDE_PROJECT_DIR/target/task-plan-guard\""
           }
         ]
       }
     ]
   }
   ```

   This hook now runs for every matching tool call of every agent, the main
   conversation included; the guard returns at once for anyone but the two
   roles.
4. **Remove the frontmatter hooks**, but only after the first live check
   below passes: delete the `hooks:` block from
   `.claude/agents/implementer.md` and `.claude/agents/ui-implementer.md`.
   Reason: in a trusted interactive session both hooks would run the guard
   twice for the same call (harmless, but two sources of truth), and the
   frontmatter one is the one that is silently skipped elsewhere. Update
   the agents' prose where it says "A hook blocks those reads" to say the
   project's settings-file hook does it, for this role, and mirror the
   wording in the `.codex/agents/{implementer,ui-implementer}.toml`
   parentheticals (Codex has no hooks; wording only).
5. `CLAUDE.md`: the layout line for `devtools/task-plan-guard/` says it runs
   from `.claude/settings.json` and guards only `implementer` and
   `ui-implementer` (by `agent_type`). `docs/plans/README.md` is the
   planner's; do not edit it.

### Rules that bind this task

- The guard's decisions for the two roles stay identical to M11's.
- Fails open as before (bad input, build failure). Never block an agent that
  is not one of the two roles.
- Each `.claude` agent and its `.codex` twin stay the same role.
- fmt clean; clippy pedantic `-D warnings`; scoped `#[expect]` only.
- **Working under the guard:** once step 3 lands, your own Bash commands
  that name the marker string are blocked; keep inputs that contain it in
  files.

## Files

Edit: `devtools/task-plan-guard/{src/lib.rs,src/main.rs,tests/cases.rs}`,
`.claude/settings.json`, `.claude/agents/implementer.md`,
`.claude/agents/ui-implementer.md`, `.codex/agents/implementer.toml`,
`.codex/agents/ui-implementer.toml`, `CLAUDE.md`.

Do not touch: other agents, `docs/plans/`, application code.

## Acceptance criteria

- **Live check, required, from real subagents** (run by the orchestrator or
  the human after the settings hook lands; record every result in the PR,
  with the trace file's lines). In a Claude Code session in this repository,
  in the cloud runtime and, if one is available, a local trusted CLI
  session, with `TASK_PLAN_GUARD_TRACE` set:
  1. an `implementer` subagent's `Read` of
     `docs/plans/project/gpui-migration.md` is **blocked** with the guard's
     message, and the trace shows `implementer`;
  2. the same subagent's `Read` of a file under `docs/plans/tasks/` is
     allowed;
  3. a `ui-implementer` subagent is blocked the same way as in 1;
  4. a `planner` subagent and the main conversation can `Read` the project
     plan (trace shows `planner` and `-`);
  5. repeat 1–4 after removing the frontmatter hooks (step 4); results
     unchanged.
  If check 1 fails because `agent_type` does not arrive, stop: do not remove
  the frontmatter hooks, and report the trace.
- `tests/cases.rs` passes with the new rows; fmt, clippy pedantic and
  `cargo test --workspace` pass on both CI jobs.
- `.claude/settings.json` still enables `impeccable`.

## Tests proving completion

New rows in `devtools/task-plan-guard/tests/cases.rs`:

- a project-plan `Read` with `agent_type: "implementer"` → blocked;
  `"ui-implementer"` → blocked;
- the same with no `agent_type`, with `"planner"`, `"reviewer"`,
  `"Explore"`, `"general-purpose"`, `"some-plugin:implementer"`, and a
  non-string `agent_type` → allowed;
- every existing row run with `agent_type: "implementer"` gives its old
  result (the table adds the field to each old case);
- `trace_line_format` (temp file) and `trace_write_failure_is_ignored`.

Command: `cargo test -p task-plan-guard`.

## Out of scope

- Any change to what the guard blocks for the two roles.
- Guarding other roles.

## Risks

- The main conversation now starts the guard on every matching tool call
  (cargo startup, tens of milliseconds once built; the first call compiles
  it).
- If a Claude Code release stops sending `agent_type`, the guard allows
  everything, silently. Repeat the live check after Claude Code upgrades
  that touch hooks.
- A session started with `--agent implementer` is guarded too, which is
  intended.
