# U1 — Shared controls for the Database Manager

Status: **superseded (2026-11-21), not rewritten:** merged into gpui-migration U2 and U3 (shared widgets in `apps/filemanager/src/widgets/`); the ten new icons are added by dbmanager U2. Do not run this plan; it is kept as the record of the design.
Project: dbmanager   Depends on: M10 (landed)

## Goal

Add the shared vocabulary the Database Manager needs to `apps/desktop/src/app/`
and `styles/controls.css`:

- a danger button variant;
- an extended `ConfirmDialog`;
- a generic `MenuButton` and a generic `ShortcutsHelp`;
- ten new icons;
- form-control styles.

Existing tools must keep working unchanged.

## Context

**The product.** FileManager is a portable Windows desktop app: a Tauri v2
shell with a React + TypeScript + Vite frontend in `apps/desktop/src/`. It
hosts independent tools: the Pipeline, the Tar Packager, the Schedule Creator,
and now the **Database Manager** (`dbmanager`), which edits the objects in a
SQLite file as typed forms. Later tasks (U2–U12) build that view. This task
builds only the shared pieces it needs.

You own `apps/desktop/src/` **except `lib/`**, which belongs to the backend
implementer: never edit it. Tools never import each other, so shared code
lives in `app/` and `styles/controls.css`, and nothing in `app/` imports from
`tools/`.

**The visual system ("The Packing List", `DESIGN.md`; normative source
`apps/desktop/src/styles/tokens.css`).** Quiet, dense, flat:

- neutrals `--bg`, `--surface`, `--surface-sunken`, `--border`, `--text` and
  `--text-muted`;
- one accent, `--accent` / `--accent-text`, for the primary action, the
  current place and focus (`--focus-ring`);
- status hues `--ok`, `--warn` and `--danger`, always paired with an icon and
  a word;
- 1 px borders and `--radius` (6 px);
- no shadow at rest; overlays use `--shadow-overlay`, and the dialog backdrop
  uses `--scrim`;
- spacing `--space-1…6` (4–32 px); type sizes `--font-size-sm/md/lg/xl`, all
  rem;
- light and dark are both first-class, and every color comes from a token
  (the only hex in `src/` is in `tokens.css`).

**No new tokens in this task.** It uses `--surface`, `--surface-sunken`,
`--border`, `--text`, `--text-muted`, `--accent`, `--accent-text`,
`--danger`, `--focus-ring`, `--shadow-overlay`, `--scrim`, `--radius`, the
spacing scale, the type sizes and `--font-mono`.

**What exists (read it before changing it):**

- `app/Button.tsx`: variants `primary | secondary | quiet`, plus `icon`,
  `iconAfter`, `iconFlip` and `busy` (busy sets `disabled` and
  `aria-busy`). Styles are in `styles/controls.css` (`.btn`, `.btn--primary`,
  `.btn--quiet`, disabled and busy rules).
- `app/ConfirmDialog.tsx`: a native `<dialog>` opened with `showModal()`.
  - Its props are `open`, `title`, `body`, `confirmLabel`, `onConfirm` and
    `onCancel`.
  - `body` renders inside `<p id=bodyId>`, the target of
    `aria-describedby`.
  - Focus starts on Cancel, Escape counts as Cancel, and focus returns to the
    opener on close.
  - Styles: `.confirm-dialog*` in `controls.css`, 28rem wide.
- `app/icons.tsx`: `IconName`, a 16 px grid with 1.5 px stroke, round caps
  and joins, `currentColor`, `aria-hidden`.
- The Tar Packager has a tool-local Recent menu (in
  `tools/tarpack/ManifestHeader.tsx`) and a tool-local
  `tools/tarpack/ShortcutsHelp.tsx`, with CSS scoped under `.tarpack` in
  `styles/tarpack.css` (classes `.menu*`, `.shortcuts*`, `kbd`). **Read them
  as the behavioural reference. Do not edit or import them.** The shared
  versions use different class names (`.menu-button*`, `.shortcuts-help*`),
  so the two never style each other.
- `styles/tokens.test.ts` asserts contrast pairs and that only
  `.tarpack .shortcuts__popover` and `.tarpack .menu__list` in `tarpack.css`
  have a shadow. Leave that assertion as it is.

### Inventory entries (this task)

| Component | File | Props | States |
| --- | --- | --- | --- |
| `Button` (extended) | `app/Button.tsx` | `variant` gains `"danger"` | default, hover, active, focus, disabled, busy |
| `ConfirmDialog` (extended) | `app/ConfirmDialog.tsx` | below | open, busy, closed |
| `MenuButton` | `app/MenuButton.tsx` | below | closed, open, empty (renders nothing), item disabled |
| `ShortcutsHelp` | `app/ShortcutsHelp.tsx` | below | closed, open |
| `Icon` (extended) | `app/icons.tsx` | ten new names | — |
| Form controls | `styles/controls.css` | `.input`, `.checkbox` | default, hover, focus, invalid, read-only, disabled |

### Danger button

`variant="danger"` adds `btn--danger`. It is used only for an action that
destroys data or discards the user's work: **Delete** and **Discard changes**.

- **Rest:** `color: var(--danger)`, `background: var(--surface)` and
  `border-color: var(--danger)`. It has no fill, so it stays calmer than
  primary and never competes with the one blue action.
- **Hover and active:** `background: var(--surface-sunken)`.
- **Disabled and busy:** the shared `.btn:disabled` rules (busy keeps full
  strength).

`--danger` on `--surface-sunken` must meet 4.5:1. Add the pair to
`tokens.test.ts`; it is about 6.0:1 in light and 8:1 in dark.

### `ConfirmDialog` (extended, backward compatible)

```ts
interface ConfirmDialogProps {
  open: boolean;
  title: string;
  body: ReactNode;                   // short description; stays the aria-describedby <p>
  confirmLabel: string | null;       // null: no confirm button, the cancel button is the only one
  onConfirm: () => void;
  onCancel: () => void;
  cancelLabel?: string;              // default "Cancel" (e.g. "Keep editing", "Close")
  confirmVariant?: "primary" | "danger";   // default "primary"
  confirmIcon?: IconName;
  confirmBusy?: boolean;             // confirm shows busy; Cancel disabled; Escape ignored
  size?: "default" | "wide";         // wide: width min(40rem, 100% - 2 * --space-5)
  children?: ReactNode;              // extra content after the description
}
```

- **`children`** render in `<div className="confirm-dialog__content">`,
  between the description and the actions. It is **not** part of
  `aria-describedby` (long impact lists must not be read as the
  description). It scrolls on its own: `max-height: min(50vh, 24rem)`,
  `overflow: auto`, a 1 px `--border` rule above and below, padding
  `--space-2 0`, and `tabIndex={0}` with a `role="region"` and an
  `aria-label` of the dialog title plus " details" when it can overflow (so
  keyboard users can scroll it).
- **Busy:** focus moves to the `<dialog>` element itself (`tabIndex={-1}`),
  because the confirm button is disabled and must not drop focus to the page.
  The `cancel` event is prevented and ignored.
- With `confirmLabel: null`, focus starts on the single button.
- With none of the new props, the markup and behaviour are identical to
  today's, and the existing tests pass unchanged.

### `MenuButton`

```ts
interface MenuItem { id: string; label: ReactNode; description?: string; title?: string;
                     disabled?: boolean; onSelect: () => void }
interface MenuButtonProps { label: string; menuLabel: string; items: MenuItem[];
                            disabled?: boolean; keyshortcut?: { aria: string; title: string } }
```

**Behaviour** (the same as the Tar Packager's Recent menu):

- The trigger is a secondary `Button` with `icon="chevron-down" iconAfter`,
  `aria-haspopup="menu"` and `aria-expanded`.
- Click, Enter or Space toggles the menu. ArrowDown on the trigger opens it on
  the first item.
- In the menu (`role="menu"`, `aria-label={menuLabel}`), the items are quiet
  buttons with `role="menuitem"` and `tabIndex={-1}`:
  - Up/Down wrap, and Home/End jump;
  - Escape closes and focuses the trigger, and stops propagation so a view's
    Escape shortcut does not also fire;
  - Tab closes and focuses the trigger;
  - a `mousedown` outside closes it;
  - selecting an item closes the menu, focuses the trigger, then calls
    `onSelect`.
- Disabled items are skipped by the arrows and have `aria-disabled="true"`.
- `items.length === 0` renders nothing.
- **Item layout:** `label` on one line, then `description` (when given) on a
  second line in `--text-muted` at `--font-size-sm`. Both wrap
  (`overflow-wrap: anywhere`). `title` carries the full text.
- **CSS (`controls.css`):**
  - `.menu-button` has `position: relative`;
  - `.menu-button__list` is absolutely positioned under the trigger
    (right-aligned, `z-index` above content), `min-width: 16rem`, `max-width:
    min(32rem, 90vw)`, `max-height: 60vh` scrolling, `--surface`, 1 px
    `--border`, `--radius`, `--shadow-overlay`, padding `--space-1`;
  - `.menu-button__item` is full width, left-aligned, padding `--space-1
    --space-2`, with the description in a `.menu-button__desc` span.

### `ShortcutsHelp` (generic)

```ts
export interface Shortcut { keys: string[][]; action: string; when: string; devOnly?: boolean }
export function ShortcutsHelp(props: { shortcuts: Shortcut[]; onOpenChange?: (open: boolean) => void }): JSX.Element;
```

The behaviour copies `tools/tarpack/ShortcutsHelp.tsx`:

- a quiet `Button` with the `keyboard` icon, the label "Keyboard shortcuts"
  and `aria-haspopup="dialog"`, toggling a native `popover="auto"`
  `role="dialog"` with `aria-label="Keyboard shortcuts"`;
- an `h2` and a table with a visually hidden caption and header (Keys /
  Action / Active when);
- chords join `<kbd>` keys with an `aria-hidden` "+", and alternatives are
  joined by "or" in `--text-muted`;
- `devOnly` rows are listed only when `useDevMode()` is true;
- Escape inside closes it, refocuses the button and stops propagation;
- the toggle handler records `data-open` (jsdom has no `:popover-open`).

Keep the one documented
`eslint-disable-next-line jsx-a11y/no-static-element-interactions` on the
wrapper span that only catches Escape.

CSS uses the classes `.shortcuts-help`, `.shortcuts-help__button`,
`__popover`, `__title`, `__table`, `__keys`, `__or`, `__when`, and
`.shortcuts-help kbd`, with the same values as the `.tarpack` versions in
`tarpack.css`:

- popover: `--surface`, 1 px `--border`, `--radius`, `--shadow-overlay`,
  padding `--space-3 --space-4`, scrolling inside itself;
- placement: anchored under the button with CSS anchor positioning
  (`anchor-name`, `position-anchor`) where `@supports` it and the window is
  at least 40rem tall, else pinned top right;
- `kbd`: inline-block, `--font-mono` at `--font-size-sm`, `--surface-sunken`,
  1 px `--border`, `--radius`, padding `0 var(--space-1)`.

### Icons

Add these names to `IconName` and draw them on the existing 16 px grid. The
paths are suggestions; keep the stroke system.

| Name | Path(s) |
| --- | --- |
| `arrow-left` | `M13 8H3M7 4L3 8l4 4` |
| `arrow-right` | `M3 8h10M9 4l4 4-4 4` |
| `lock` | `<rect x="3.25" y="7.25" width="9.5" height="6.5" rx="1"/>` + `M5.25 7.25V5a2.75 2.75 0 015.5 0v2.25` |
| `plus` | `M8 3v10M3 8h10` |
| `trash` | `M2.75 4.25h10.5M6.25 4.25V2.75h3.5v1.5M4.25 4.25l.6 8.6a1 1 0 001 .9h4.3a1 1 0 001-.9l.6-8.6` |
| `search` | `<circle cx="7" cy="7" r="4.25"/>` + `M10.25 10.25l3.25 3.25` |
| `chevron-left` | `M10 3.5L5.5 8l4.5 4.5` |
| `chevron-right` | `M6 3.5L10.5 8 6 12.5` |
| `undo` | `M5.5 3.5L2.75 6.25 5.5 9M2.75 6.25h6.5a3.75 3.75 0 010 7.5H7` |
| `check` | `M3.5 8.5l3 3 6-7` |

### Form controls (`controls.css`)

These classes are used by the Database Manager. Other tools may adopt them
later, but do not migrate them in this task.

- **`.input`** (on `input`, `textarea`, `select`):
  - `min-height: 2rem`, padding `var(--space-1) var(--space-2)`, `font:
    inherit`, `--font-size-md`, `color: var(--text)`, `background:
    var(--surface)`, 1 px `--border`, `--radius`, `min-width: 0`;
  - `::placeholder` in `--text-muted`;
  - `:focus-visible`: 2 px `--focus-ring` outline, offset 1px;
  - `[aria-invalid="true"]`: `border-color: var(--danger)`;
  - **read-only** (`[readonly]`, `[aria-readonly="true"]`): `background:
    var(--surface-sunken)`, `border-color: transparent`, full-strength
    `--text`, `cursor: text`. It is never dimmed: read-only is marked, not
    faded;
  - `:disabled`: `opacity: 0.6`, `cursor: not-allowed`;
  - `textarea.input`: `resize: vertical`, `min-height: 5rem`, line-height 1.4;
  - `.input.num`: tabular numerals (the existing `.num` class).
- **`.checkbox`** (on `input[type=checkbox]`): `accent-color: var(--accent)`,
  `width: 1rem`, `height: 1rem` (rem, so it scales at 200%), and `margin: 0`.
  It takes the global focus ring.

### `DESIGN.md`

`DESIGN.md` already describes these components as **Planned (dbmanager U1)**
(written by the ui-designer). When they land, change each of those labels to
**Landed (dbmanager U1)** and correct any detail that differs from what you
built. Change nothing else in `DESIGN.md`.

## Files

Edit:

- `apps/desktop/src/app/Button.tsx`, `app/Button.test.tsx`;
- `app/ConfirmDialog.tsx`, `app/ConfirmDialog.test.tsx`;
- `app/icons.tsx`;
- `styles/controls.css`;
- `styles/tokens.test.ts` (the one new contrast pair);
- `DESIGN.md` (labels only).

Create:

- `app/MenuButton.tsx`, `app/MenuButton.test.tsx`;
- `app/ShortcutsHelp.tsx`, `app/ShortcutsHelp.test.tsx`.

Do not touch `lib/`, `tools/` (any tool), `tokens.css`, `tarpack.css`,
`schedule.css`, `pipeline.css`, `App.tsx` or the registry.

## Skill

- Run `/impeccable polish` on the new and changed components once they work:
  the launcher loads `PRODUCT.md` and `DESIGN.md`; run it from `apps/desktop`
  with `--target src/app/ConfirmDialog.tsx`.
- Read the skill's `reference/craft-floor.md` before the first UI edit.
- This **extends the established world**: no new visual language, no new
  tokens.
- Before declaring done, run `/impeccable audit` on the changed files, then
  the detector once:
  `"<skill dir>/scripts/impeccable" detect --json apps/desktop/src/app
  apps/desktop/src/styles/controls.css`.
- If the skill is not installed, install it with `npx impeccable install`,
  or follow the command reference docs from `github.com/pbakaus/impeccable` by
  hand, and say so in your report.

## Acceptance criteria

**Existing behaviour:**

- `npm run typecheck && npm run lint && npm run test` pass. Every existing
  test passes unchanged, including the Tar Packager, Schedule and Pipeline
  tests and `tokens.test.ts`.

**Danger button:**

- It renders `btn btn--danger`.
- `--danger` on `--surface-sunken` ≥ 4.5:1 is asserted for light and dark.

**`ConfirmDialog`:**

- `cancelLabel` replaces "Cancel".
- `confirmLabel={null}` renders one button, and focus starts on it.
- `confirmVariant="danger"` renders a danger button.
- `confirmBusy`:
  - the confirm button has `aria-busy="true"`;
  - Cancel is disabled;
  - Escape does not call `onCancel`;
  - focus is on the dialog element.
- `children` render after the description, outside the `aria-describedby`
  element.
- `size="wide"` adds `confirm-dialog--wide`.

**`MenuButton`:**

- Keyboard: ArrowDown on the trigger opens the menu and focuses the first
  item; Up/Down wrap; Home/End jump; disabled items are skipped; Escape
  closes and refocuses the trigger; Tab closes.
- A click outside closes it.
- Selecting an item calls `onSelect` once, after the menu closes.
- With no items, it renders nothing.
- `aria-expanded` follows the state.

**`ShortcutsHelp`:**

- It lists the given rows, and hides `devOnly` rows unless developer mode is
  on.
- Escape closes it and refocuses the button.

**Icons, controls and lint:**

- Every icon renders with `aria-hidden="true"`.
- The `.input` read-only rule uses `--surface-sunken`, not opacity.
- `npm run lint` reports no jsx-a11y finding; the only disable comment is the
  documented one.
- Keyboard: every new control is reachable and operable with the keyboard
  alone, and shows the global focus ring.

## Tests proving completion

- `Button.test.tsx`: `danger variant`.
- `ConfirmDialog.test.tsx`:
  - the existing tests;
  - `custom cancel label`;
  - `single button when confirmLabel is null`;
  - `danger confirm`;
  - `busy blocks cancel and escape and keeps focus in the dialog`;
  - `children are outside the description`;
  - `wide size`.
- `MenuButton.test.tsx`:
  - `opens with ArrowDown and focuses first item`;
  - `arrows wrap and skip disabled`;
  - `Escape and Tab close and refocus`;
  - `outside click closes`;
  - `select calls onSelect after closing`;
  - `renders nothing without items`.
- `ShortcutsHelp.test.tsx`:
  - `lists rows`;
  - `devOnly rows only in developer mode`;
  - `Escape closes and refocuses`.
- `tokens.test.ts`: the new `--danger on --surface-sunken` pair.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Buttons:** rest, hover, active, focus, disabled, busy.
- **Dialog:** open, busy, single-button, wide with scrolling content.
- **Menu:** closed, open, empty, disabled item.
- **Inputs:** default, focus, invalid, read-only, disabled.

## Out of scope

- Any Database Manager view (U2 onwards).
- Migrating the Tar Packager's menu or shortcuts help, or the Schedule
  inputs, to the shared ones.
- New tokens or token values.
