# Plans

Two files per change, named for the change rather than numbered:

- `<slug>.md` — the backend pipeline, written by `planner`.
- `<slug>-ui.md` — the UI pipeline, written by `ui-designer`.

A change with no visual surface has only the first. A purely visual change has
only the second.

## Backend milestones

```markdown
## Milestone N — <goal>

**Goal.** One sentence.
**Depends on.** Milestone N-1, or "none". Note "depends on UI plan" where it applies.
**Components / likely files.** ...
**Architectural change.** ...
**Acceptance criteria.** Checkable statements, not intentions.
**Tests proving completion.** Named tests, with the command that runs them.
**Existing tests to refactor.** ...
**Compatibility / regression risk.** ...
```

## UI plans

A UI plan opens with the design brief, the design tokens it touches (values for
both themes), and the component inventory, then lists its tasks:

```markdown
## UI task N — <goal>

**Goal.** One sentence.
**Depends on.** UI task N-1, and backend milestone M, or "none".
**Files.** Exact paths to create or edit.
**Skill.** The impeccable skill the implementer applies.
**Acceptance criteria.** Including accessibility and keyboard checks.
**Tests proving completion.** ...
**States covered.** Loading / empty / error / success.
```

Keep plans after the work merges — they are the record of why the architecture
and the interface look the way they do.
