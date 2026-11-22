# U7 — Make the "repeat on same count" announcement test deterministic

Status: approved (2026-11-22; the human asked for it before PR #33 merges)
Project: tarpack   Depends on: U6 (landed)

## Goal

Make the test "open and reload announce, repeat on same count, and clear
announces No errors" in
`apps/desktop/src/tools/tarpack/TarpackView.errors.test.tsx` pass every
time, without weakening what it proves: a reload that gives the same error
count is announced again.

## Context

The Tar Packager's React view (`apps/desktop/src/tools/tarpack/TarpackView.tsx`)
announces results through a visually hidden live region,
`<Announcer text={announcement} />` (`role="status"`, `aria-live="polite"`,
`data-testid="tarpack-announcer"`). The same sentence twice in a row would
not be re-read by a screen reader, so `announce(text)` clears first and sets
the text on the next frame:

```ts
const announce = useCallback((text: string) => {
  cancelAnimationFrame(frame.current);
  setAnnouncement("");
  frame.current = requestAnimationFrame(() => setAnnouncement(text));
}, []);
```

The test reloads a manifest that gives the same sentence ("… 2 files will be
left out …"), records the announcer's text with a `MutationObserver`, waits
for the sentence, and asserts `expect(seen).toContain("")  // cleared, then set again`
(line 72).

**The failure.** CI run 44980645274, linux job, `npm run test`: line 72
failed with `expected [] to include ''`. It passed in every earlier run on
the branch; a similar one-off linux frontend failure was seen once before.

**Likely mechanism (check it; do not take it on trust).**
`setAnnouncement("")` is called from an async continuation (after the
mocked `reloadManifest` resolves), so React 18 batches it and renders on a
later scheduler task. jsdom's `requestAnimationFrame` is a ~16 ms timer. On
a slow, busy runner the rAF callback can fire **before** React commits the
`""` render; React then batches `""` and the sentence into one render, the
DOM text goes from the sentence to the same sentence, nothing changes, and
the observer records nothing (`seen` is `[]`, which is what CI printed).

If that is right, it is not only a test-observation race: in a real webview
whose main thread is busy for more than a frame after the clear, the same
batching would skip the clear, and a screen reader would not re-announce.
That is a **component race**.

### What to do

1. **Diagnose first.** Confirm or refute the mechanism above. For example,
   delay React's commit of the clear (a slow fake for the scheduler, or a
   busy-wait in a `useLayoutEffect` only in a scratch copy of the test) and
   see whether the clear disappears from the DOM. Record what you found in
   the PR.
2. **If the component can lose the clear** (the clear and the sentence can
   be committed in one render), **stop and report**: do not change the
   component in this task. Report the evidence and the fix you would
   propose (for example committing the clear synchronously with
   `flushSync(() => setAnnouncement(""))` before scheduling the frame), so the
   planner can decide. Leave the test as it is.
3. **If the component cannot lose the clear** (the clear is always committed
   before the frame callback runs, and only the observation is racy), fix
   the test only. Make it wait on the real mechanism explicitly instead of
   on wall-clock luck. Pick one of these, or an equivalent that keeps the
   same proof:
   - control the frame: `vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] })`
     (keep real timers for what `userEvent` and `waitFor` need, or use
     `userEvent.setup({ advanceTimers: vi.advanceTimersByTime })`); after the
     Reload click, `await act(async () => {})` until the announcer is `""`
     and assert that, then `act(() => vi.advanceTimersToNextFrame())` (or
     `advanceTimersByTime(16)`) and assert the sentence; or
   - assert the re-announcement by identity: capture the announcer's text
     node before the reload, and after it assert the text is the sentence
     **and** the clear was observed, waiting for each step with `waitFor`
     on a condition rather than collecting observer callbacks.

   Whatever you choose, the test must still fail if `announce` stopped
   clearing (check it: temporarily remove `setAnnouncement("")` in a scratch
   change, confirm the test fails, restore it).

### Rules that bind this task

- Scope: **only** `TarpackView.errors.test.tsx`. No change to
  `TarpackView.tsx`, other components, other tests, or test setup files. If
  a fix seems to need any of those, stop and report.
- Keep the test's other assertions (open announces, clear announces "No
  errors.", no report region) unchanged.
- The frontend stays type-checked and lint clean (oxlint with jsx-a11y).
- Tests never touch real files (this one does not).

## Files

Edit: `apps/desktop/src/tools/tarpack/TarpackView.errors.test.tsx`.

Do not touch: anything else.

## Skill

None (test determinism; no visual change). Do not run `impeccable`.

## Acceptance criteria

- Either the PR reports a component race with evidence and a proposed fix,
  and changes nothing; or:
  - the test passes **50 runs in a row** locally, e.g.
    `cd apps/desktop && for i in $(seq 50); do npx vitest run src/tools/tarpack/TarpackView.errors.test.tsx || break; done`
    (or vitest's `--repeat` where supported); the PR states the count;
  - the scratch check above shows the test fails when the clear is removed;
  - `npm run typecheck`, `npm run lint` and `npm run test` pass at the repo
    root.
- The diagnosis is recorded in the PR either way.

## Tests proving completion

- `TarpackView.errors.test.tsx`, all cases, 50 consecutive passes.
- `npm run typecheck && npm run lint && npm run test`.

## States covered

Not applicable (no UI state changes).

## Out of scope

- Any change to the announcer or `announce` (report instead).
- The GPUI app, where announcements are not rendered.
- Other flaky tests; report any you see.
