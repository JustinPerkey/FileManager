# FileManager

A standalone application repository. It owns its own history, CI, and agent
chain, and follows the same five-role chain as the Warner Robins repositories.

## Status

The stack is not yet chosen. Until it is, this file is the placeholder the agent
chain reads, and the rules below are the ones that hold regardless of stack.

The first `planner` run establishes: the stack, the module layout, the
core/UI boundary, and the verification commands — and updates the Layout,
Commands, and Invariants sections here as part of that plan's first milestone.
No feature work starts before that milestone lands.

## Layout

- `.codex/agents`, `.claude/agents` — planner / ui-designer / implementer /
  ui-implementer / reviewer roles, defined once per runtime and kept in sync.
- `docs/plans/` — approved implementation plans and UI plans.
- Application layout: to be established.

## Commands

To be established. Whatever the stack, the chain expects three things to exist
and be named here: a test command, a lint command that fails on warnings, and a
typecheck or compile command.

## Invariants

- A type that crosses the core/UI boundary is defined once and mirrored
  explicitly on the other side. Change both or neither.
- Destructive filesystem operations — delete, overwrite, move onto an existing
  path — are never silent. They are confirmed or recoverable, and a failure part
  way through a batch is reported, not swallowed.
- Tests never touch real user files. Every test that exercises the filesystem
  runs inside a temporary directory it creates and removes.
- Paths are handled as the platform's path type, not as display strings. Code
  that renders a path does not assume it is valid UTF-8.
- No secrets in the repository. Configuration comes from the environment, with a
  checked-in `.env.example` documenting every key.
- Accessibility is a requirement, not a polish item. Every interactive element
  is keyboard reachable and labelled.
- Each `.claude` agent and its `.codex` counterpart are the same role written
  for two runtimes. Change one, change the other.
- No code change is complete until the `reviewer` has seen it. An implementer
  run ends by handing its diff to the reviewer.

## Working style

Substantial changes go through the chain: planner → (ui-designer, when the
change has a visual surface) → approval → implementer and ui-implementer, one
milestone or task per run → reviewer.

UI work uses the `impeccable` skill family: `ui-designer` shapes and critiques,
`ui-implementer` crafts and polishes, both audit before declaring done. The
backend `implementer` does not make visual decisions; `ui-implementer` does not
touch backend source. The two meet at the UI-facing half of the core contract,
which the `implementer` owns.
