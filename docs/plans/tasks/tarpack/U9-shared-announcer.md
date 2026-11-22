# U9 — One shared announcer hook; fix the repeat-announcement race in Pipeline and Schedule Creator

Status: approved (2026-11-22; the human approved the fix as its own PR, with one shared hook)
Project: tarpack (shared app code; follows U8)   Depends on: U8 (landed in main via PR #33)

## Goal

Move the race-free announcer mechanism the Tar Packager uses into one
shared hook in `apps/desktop/src/app/`, and use it in the Tar Packager,
Pipeline and Schedule Creator views. A repeated sentence is then
re-announced in all three, and each view gets a deterministic regression
test.

## Context

Each tool view announces results through a visually hidden live region
(`role="status"`, `aria-live="polite"`). A screen reader does not re-read an
unchanged region, so an announcement clears the region and then sets the
text on the next frame.

**The race.** The old mechanism, still in two views, is:

```ts
const announce = useCallback((text: string) => {
  cancelAnimationFrame(frame.current);
  setAnnouncement("");
  frame.current = requestAnimationFrame(() => setAnnouncement(text));
}, []);
useEffect(() => () => cancelAnimationFrame(frame.current), []);
```

When it runs from an async continuation, React 18 only *schedules* the `""`
render. If the frame fires first, React batches `""` and the sentence into
one render, the DOM text never changes, and a screen reader does not
re-announce. It was reproduced in the Tar Packager (fake rAF, act
environment off, frame advanced before React's scheduler: the observer saw
nothing). The reviewer confirmed the same race here:

- `apps/desktop/src/tools/pipeline/PipelineView.tsx` lines 95–100 (with
  async callers at about lines 163, 214 and 238: a folder error, a step's
  result, the end of Run all), live region at about line 282;
- `apps/desktop/src/tools/schedule/ScheduleView.tsx` lines 71–76 (async
  callers at about lines 153, 176 and 196: a typed path's error, a drop, the
  apply result), live region at about line 209.

**The fix that landed** in `apps/desktop/src/tools/tarpack/TarpackView.tsx`
(about lines 91 and 105–116):

```ts
type AnnouncerState = { text: string; pending: string; id: number };
const [announcer, setAnnouncer] = useState<AnnouncerState>({ text: "", pending: "", id: 0 });
// Clear now, set on a frame scheduled only after the clear has committed.
const announce = useCallback((text: string) => {
  setAnnouncer((a) => ({ text: "", pending: text, id: a.id + 1 }));
}, []);
const announceId = announcer.id;
useEffect(() => {
  if (announceId === 0) return;
  const f = requestAnimationFrame(() => setAnnouncer((a) => ({ ...a, text: a.pending })));
  return () => cancelAnimationFrame(f);
}, [announceId]);
```

The frame is scheduled only from the effect of the commit that put `""` in
the DOM, so no batching can merge the two. It works from any call site
(events, async continuations, timers, effects, an imperative handle)
because `announce` is a plain functional state update. A newer `announce`
cancels the older pending frame through the effect cleanup.

Its regression test is in
`apps/desktop/src/tools/tarpack/TarpackView.errors.test.tsx`: "a repeated
sentence is re-announced even when the frame fires before React renders".
Read it; the two new tests follow it.

### What to build

1. **`apps/desktop/src/app/useAnnouncer.ts`:**

   ```ts
   /** A live-region announcer that re-announces a repeated sentence: clear,
    *  then set on a frame scheduled only after the clear has committed. */
   export function useAnnouncer(): { text: string; announce: (text: string) => void };
   ```

   - It holds the exact mechanism above, moved, not rewritten.
   - `announce` is stable (`useCallback` with no dependencies), so existing
     dependency arrays and the Tar Packager's `useImperativeHandle` stay
     valid.
   - The rendering of the region stays in each view. Do not change the
     region's markup, attributes, class or test ids.
2. **TarpackView:** replace its local state, `announce` and effect with the
   hook. Behaviour, markup and the `actionsRef` handle are unchanged. Remove
   the `AnnouncerState` type if it has no other use.
3. **PipelineView and ScheduleView:**
   - replace the old `announce`, its `frame` ref, the `announcement` state
     and the unmount cleanup effect with the hook;
   - the region renders the hook's `text`.

   Every existing call site keeps calling `announce(text)`.
4. **Hook unit test, `apps/desktop/src/app/useAnnouncer.test.tsx`** (with
   `renderHook`):
   - the first state is `""`;
   - `announce` commits `""` before the frame and the text after it;
   - a second `announce` before the frame replaces the pending text, and only
     the latest is shown;
   - unmounting cancels the frame;
   - *optional, recommended:* one hook-level race test, so the hook's own
     file catches the race without a view. With `IS_REACT_ACT_ENVIRONMENT`
     saved, set to `false`, and restored to its saved value in `finally`,
     and a rAF stub that fires frames in a microtask (its own `Map` with
     high ids, as in item 5), call `announce` with the current text from an
     async continuation, for example after `await Promise.resolve()`. Then
     assert with a `MutationObserver` on a rendered `<div>{text}</div>`
     that `""` was observed before the text. If you add it, include it in
     the revert check: it must fail against a hook that schedules the frame
     directly in `announce`.
5. **Regression tests:** one in `PipelineView.test.tsx` and one in
   `ScheduleView.test.tsx`, modelled on the Tar Packager test.
   - Trigger the **same** sentence twice from an **async** call site:
     - Pipeline: run the same step twice with the same result. Each run
       announces "<title>: <status>." after the awaited command; use the
       view's existing mocks.
     - Schedule: commit two **different** relative paths in the schedule
       file field, for example `week.txt` and then `month.txt` (type, then
       Enter, for each). The same path twice cannot be sent: the backend
       keeps the typed path in the slot, and the field's unchanged-path
       guard sends nothing on a second Enter. Each commit announces the
       same sentence, the `NotAbsolute` message from `errorMessages.ts`
       ("Enter the full path, …"), after the awaited `setText`. The
       `setText` mock must behave like the real backend: it echoes the
       typed path back in the slot, with a `NotAbsolute` error.
   - **rAF stub:**
     - keep its own `Map<number, FrameRequestCallback>`, with ids that start
       far above jsdom's (for example from `1_000_000`), so cancelling a real
       frame never hits a stub frame and the reverse;
     - fire each frame in a microtask, so it runs after the `announce` call
       and before React's scheduler, which is a macrotask;
     - make `cancelAnimationFrame` delete from the Map, or forward unknown
       ids to the real function.
   - **Act environment:** save `globalThis.IS_REACT_ACT_ENVIRONMENT` and set
     it to `false` for the test only. In `finally`, restore it to the
     **saved value** (not to `true` or `undefined`), together with the real
     `requestAnimationFrame` and `cancelAnimationFrame`.
   - **Assertions,** with a `MutationObserver` on the region (locate it with
     `getByRole("status")` within the view, since these regions have no
     test id): `""` was observed, the last observed text is the sentence,
     and the `""` came before it. Use only condition-based waits, plus one
     bounded settle like the Tar Packager test's 50 ms.

### Rules that bind this task

- **Scope:**
  - create `apps/desktop/src/app/useAnnouncer.ts` and its test;
  - edit `TarpackView.tsx`, `PipelineView.tsx`, `ScheduleView.tsx`, and
    their test files (only to add the regression tests and, if needed, adapt
    a test that relied on the removed internals; say which and why).
  - Nothing else: no test setup files, no `lib/`, no styles.
- **No visible change.** Each region stays visually hidden, with the same
  node, attributes and ids. The ui-implementer owns these files; the backend
  `lib/` is not touched.
- Every interactive element stays keyboard reachable and labelled; nothing
  here adds one.
- `npm run typecheck`, `npm run lint` (oxlint, jsx-a11y) and `npm run test`
  pass.

## Files

Create: `apps/desktop/src/app/useAnnouncer.ts`,
`apps/desktop/src/app/useAnnouncer.test.tsx`.

Edit:

- `apps/desktop/src/tools/tarpack/TarpackView.tsx`,
  `apps/desktop/src/tools/pipeline/PipelineView.tsx`,
  `apps/desktop/src/tools/schedule/ScheduleView.tsx`;
- `apps/desktop/src/tools/pipeline/PipelineView.test.tsx`,
  `apps/desktop/src/tools/schedule/ScheduleView.test.tsx`, and
  `apps/desktop/src/tools/tarpack/TarpackView*.test.tsx` only if adapting
  is needed.

Do not touch: `apps/filemanager/`, `crates/`, `docs/plans/`.

## Skill

None (no visual change). Do not run `impeccable`.

## Acceptance criteria

- One announcer implementation exists (`useAnnouncer`). No view keeps its
  own frame ref, rAF call or `cancelAnimationFrame` for announcements.
- **Revert check, per view.** Temporarily restore that view's old
  `announce` (clear, then rAF set, no hook) in a scratch change: its new
  regression test fails. With the hook, it passes. Do this for Pipeline and
  Schedule separately, and for the Tar Packager test against a hook whose
  frame is scheduled directly in `announce`. Show the runs in the PR.
- **50 consecutive passes per file,** run separately for
  `PipelineView.test.tsx`, `ScheduleView.test.tsx`,
  `TarpackView.errors.test.tsx` and `useAnnouncer.test.tsx`, e.g.
  `cd apps/desktop && for i in $(seq 50); do npx vitest run src/tools/pipeline/PipelineView.test.tsx || break; done`.
  The PR states each count.
- **Alone:** each new regression test passes when run by itself
  (`npx vitest run <file> -t "<test name>"`).
- **Shuffled:** `npx vitest run --sequence.shuffle` passes 10 times in a row
  for the three view test files together. Use a different seed each run
  (`--sequence.seed=<n>`), and list the seeds in the PR.
- `npm run typecheck && npm run lint && npm run test` pass at the repo
  root.
- The existing tests that check announcements still pass unchanged:
  - Tar Packager: "open and reload announce, repeat on same count, and
    clear announces No errors";
  - Schedule: "dropping files sets them and announces what was set" and "a
    typed path's error is announced".

## Tests proving completion

- `useAnnouncer.test.tsx`.
- The new regression test in `PipelineView.test.tsx` and in
  `ScheduleView.test.tsx`.
- The existing Tar Packager regression test and every existing test of the
  three views.
- The loops, alone runs and shuffled runs above, then
  `npm run typecheck && npm run lint && npm run test`.

## States covered

Not applicable (announcer timing only).

## Out of scope

- Any visible change, any other component, the GPUI app. The GPUI port
  renders no announcements (UI plan decision V2), so this code lives only
  until the Tauri frontend is removed at the cutover.
