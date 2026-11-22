# U8 — Fix the Tar Packager announcer race: a repeated sentence is always re-announced

Status: approved (2026-11-22; follow-up to U7, which found the race in the component)
Project: tarpack   Depends on: U7 (landed: diagnosis only, no change)

## Goal

Make the Tar Packager's live-region announcer re-announce a repeated
sentence every time, whatever the call site and however busy the main
thread, and prove it with a deterministic regression test.

## Context

`apps/desktop/src/tools/tarpack/TarpackView.tsx` (lines 107–111 today)
announces through a visually hidden live region,
`<Announcer text={announcement} />` (`role="status"`, `aria-live="polite"`,
`data-testid="tarpack-announcer"`, lines ~624–630). A screen reader does not
re-read an unchanged live region, so `announce(text)` clears it and sets the
text on the next frame:

```ts
const announce = useCallback((text: string) => {
  cancelAnimationFrame(frame.current);
  setAnnouncement("");
  frame.current = requestAnimationFrame(() => setAnnouncement(text));
}, []);
```

**The race (found and reproduced in U7).** When `announce` runs from an async
continuation (after an awaited command, the usual case), React 18 only
**schedules** the `""` render. If the frame fires first, React batches `""`
and the sentence into one render, the DOM text never changes, and a screen
reader does not re-announce. U7 reproduced it with fake
`requestAnimationFrame`, `globalThis.IS_REACT_ACT_ENVIRONMENT = false`, and
the frame advanced before React's scheduler ran: a `MutationObserver` on the
announcer recorded nothing (`[]`). CI run 44980645274 failed the existing
test "open and reload announce, repeat on same count, and clear announces No
errors" (`TarpackView.errors.test.tsx`, line 72,
`expect(seen).toContain("")`) the same way.

**Call sites** (audit them): `apply` (async command results), the
`onManifestChanged` handler, the drop and build result paths, a
`setTimeout` for "Matching dropped files…", and the `actionsRef` handle
(`useImperativeHandle`, so `announce` is callable from outside). U7 warned
that `flushSync`, the first fix that comes to mind, warns and does not flush
when called during render or inside an effect, so it may not hold
everywhere.

### The fix (chosen): schedule the frame only after the clear has committed

Keep clear-then-set, but move the frame scheduling into an effect that runs
**after** React has committed the clear:

- `announce(text)` makes one state update: `{ text: "", pending: text, id: id + 1 }`
  (a single `useState` or `useReducer` object; it is an ordinary state
  update, valid from events, async continuations, timers, effects and the
  imperative handle).
- A `useEffect` keyed on `id` runs after the commit that put `""` in the DOM.
  It schedules `requestAnimationFrame(() => setState({ text: pending, … }))`
  and cancels that frame in its cleanup (so a newer `announce` replaces an
  older pending sentence, as today's `cancelAnimationFrame` does).
- The `Announcer` renders `state.text`.

Why this one:

- It does not depend on commit timing. The frame cannot fire before the
  clear is in the DOM, because it is only scheduled from the effect of that
  commit, so no batching can merge the two.
- It holds at every call site. There is no `flushSync`, so no
  effect-phase or render-phase restriction and no call-site audit to keep
  correct as the code changes.
- It keeps today's DOM behaviour exactly: one live-region node, cleared,
  then set. Screen readers see the same sequence, and the existing test's
  "cleared, then set again" assertion stays the proof unchanged.
- The rejected options:
  - Two alternating regions, or a node replaced by a key change: screen
    readers often ignore a live region that is inserted with its content
    already in it, and the existing test's single test id would have to
    change.
  - An invisible alternating suffix: some readers speak or stumble on
    zero-width or no-break characters, and the clear the test proves would
    no longer exist.

If the shared pattern exists as a helper, put the fix there. Today it does
not: `PipelineView.tsx` and `ScheduleView.tsx` have their own copies. Those
two copies are **out of scope**; report that they carry the same latent race.

### Tests

- **Keep** "open and reload announce, repeat on same count, and clear
  announces No errors" unchanged. It is the behavioural proof (the clear is
  observed, then the sentence).
- **Add a deterministic regression test** in `TarpackView.errors.test.tsx`,
  modelled on U7's reproduction (for example
  `"a repeated sentence is re-announced even when the frame fires before React renders"`):
  - fake `requestAnimationFrame` and `cancelAnimationFrame` (vitest fake
    timers with `toFake` limited to them, or a manual stub that queues
    callbacks);
  - `globalThis.IS_REACT_ACT_ENVIRONMENT = false` for this test only,
    restored in `finally` / `afterEach`;
  - mount with a manifest giving a sentence, then trigger an `announce` of
    the **same** sentence from an async continuation (a Reload whose
    mocked `reloadManifest` resolves the same session);
  - **advance the frame before React's scheduler has run** (flush the
    queued rAF callbacks synchronously right after the promise resolves),
    then let the scheduler run (`await new Promise(r => setTimeout(r, 0))`
    or a scheduler flush), then advance frames again;
  - assert, with a `MutationObserver` on the announcer, that `""` was
    observed and then the sentence again.
- **Prove the test bites:** with the fix reverted (today's `announce`),
  the new test fails. Show this in the PR (a scratch run).

### Rules that bind this task

- Scope: `TarpackView.tsx` (`announce`, its state, `Announcer`), its tests
  (`TarpackView*.test.tsx`), and a shared announce helper if one existed
  (none does). No other component, no test setup file, no other tool.
- No visible change: the region stays visually hidden, one node, same
  attributes and test id.
- Every interactive element stays keyboard reachable and labelled; nothing
  here adds one.
- `npm run typecheck`, `npm run lint` (oxlint, jsx-a11y) and `npm run test`
  pass.

## Files

Edit: `apps/desktop/src/tools/tarpack/TarpackView.tsx`,
`apps/desktop/src/tools/tarpack/TarpackView.errors.test.tsx` (new test
only), and other `TarpackView*.test.tsx` only if an existing test relied on
the old internals (say which and why).

Do not touch: `PipelineView.tsx`, `ScheduleView.tsx`, `apps/desktop/src/app/`,
`apps/filemanager/`, `docs/plans/`.

## Skill

None (no visual change). Do not run `impeccable`.

## Acceptance criteria

- `TarpackView.errors.test.tsx` passes **50 consecutive runs**:
  `cd apps/desktop && for i in $(seq 50); do npx vitest run src/tools/tarpack/TarpackView.errors.test.tsx || break; done`
  (the PR states the count).
- The new regression test fails with the fix reverted and passes with it.
- The existing "repeat on same count" test is unchanged and passes.
- `npm run typecheck && npm run lint && npm run test` pass at the repo root.
- The PR lists the audited call sites and confirms that each goes through
  the new path.

## Tests proving completion

- The new regression test, the unchanged "repeat on same count" test, and
  the rest of `TarpackView*.test.tsx`.
- `npm run typecheck && npm run lint && npm run test`.

## States covered

Not applicable (announcer timing only).

## Out of scope

- The same pattern in the Pipeline and Schedule Creator views (report it).
- The GPUI app: it does not render announcements (UI plan decision V2), so
  this fix lives only until the Tauri frontend is removed at the cutover.
