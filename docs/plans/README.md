# Plans

Plans come in two categories, kept in separate directories:

```
docs/plans/
  project/<slug>.md          project plan: backend and architecture   (planner)
  project/<slug>-ui.md       project plan: design                     (ui-designer)
  tasks/<slug>/M<n>-<name>.md   one task plan per backend milestone   (planner)
  tasks/<slug>/U<n>-<name>.md   one task plan per UI task             (ui-designer)
```

A change with no visual surface has no `-ui.md` project plan and no `U` tasks.
A purely visual change has only the UI plans.

## Project plans

A project plan is the whole-change picture, and it is where decisions are made:

- the request;
- the architecture and the reasons for it;
- the cross-cutting rules;
- the design brief and tokens;
- the ordered index of task plans;
- the open questions for the human.

It is read by the planner, the ui-designer, the reviewer, the orchestrator, and
the human. **It is never read by an implementer.**

## Task plans

A task plan is the complete brief for exactly one `implementer` or
`ui-implementer` run. It is **self-contained**: everything the implementer needs
that is not already in the code or in `CLAUDE.md` is written into the task plan
itself. That includes:

- the excerpt of a spec it implements;
- the rules that bind it;
- the files it owns;
- how it is proved done.

A task plan never says "see the project plan". It may point at code, at
`CLAUDE.md`, or at a reference doc that an earlier task created outside
`docs/plans/` (for example `docs/tarpack-manifest.md`).

Implementers are handed the path of one task plan and read no other file under
`docs/plans/`. For the Claude runtime a `PreToolUse` hook enforces this
(`.claude/hooks/task-plan-only.mjs`). Codex has no hooks, so there the agent
instructions carry the rule on their own.

When a project plan changes, its author updates every task plan the change
reaches, in the same commit. The reviewer checks that the task plans still agree
with the project plan.

### Backend task plan

```markdown
# M<n> — <goal>

Status: awaiting approval | approved | in progress | done
Project: <slug>   Depends on: M<k> (landed), or "none"

## Goal
One sentence.

## Context
Everything needed from the project plan, restated: the spec excerpt, the rules
that bind this task, and the code from earlier tasks it builds on (by path).

## Files
What to create or edit. The boundary this task must not cross.

## Acceptance criteria
Checkable statements, not intentions.

## Tests proving completion
Named tests, and the command that runs them.

## Out of scope
What this task must not do, including what later tasks will do.

## Risks
Compatibility and regression concerns.
```

### UI task plan

```markdown
# U<n> — <goal>

Status: ...
Project: <slug>   Depends on: U<k>, M<k>

## Goal
## Context
The brief excerpt, the tokens it uses (by name; values live in tokens.css
once U1 lands), the inventory entries for its components, and the lib/ API it
calls.
## Files
## Skill
The impeccable skill to apply.
## Acceptance criteria
Including accessibility and keyboard checks.
## Tests proving completion
## States covered
Loading / empty / error / success.
## Out of scope
```

Keep both kinds of plan after the work merges. They are the record of why the
architecture and the interface look the way they do.
