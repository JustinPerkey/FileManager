# U1 — App shell, design tokens, and tool navigation

Status: awaiting approval
Project: tarpack   Depends on: M1 (landed)

## Goal

Build the window layout, the light and dark design tokens, and a sidebar that
lists tools from the tool registry. Every later view sits inside this frame.

## Context

**The product.** FileManager is a Windows desktop utility built from several
independent tools. The Tar Packager is the first. Users are developers and
testers who use it many times a day.

**The stack (decided).** A Tauri v2 shell with a React + TypeScript + Vite
frontend in `apps/desktop/src/`. You own everything under `apps/desktop/src/`
except `lib/`, which the backend implementer owns.

**The register.** A quiet, dense, trustworthy utility, like a good build tool.
It shows everything that matters at a glance, and nothing decorative.

**Layout.** A single window with:

- a narrow **tool sidebar** on the left: the home for future tools, with one
  entry today;
- a **main area** filling the rest, where the active tool's view renders.

The tool view will later have a header, a scrolling table, and a sticky bottom
bar. The main area must therefore give its child the full height, with the
child owning its own scrolling.

**What M1 provides.**

- `apps/desktop/src/App.tsx`: a bare placeholder, which you replace.
- `apps/desktop/src/tools/registry.ts`: exports
  `tools: { id, label, view }[]`, currently empty.
- `apps/desktop/src/lib/`: implementer-owned. Do not edit it.
- The Tauri minimum window size is 800×560.
- `npm run lint` already includes `eslint-plugin-jsx-a11y` (recommended
  rules), and fails on any finding. Fix findings in the markup. Do not disable
  the rules.
- `src/test-setup.ts` already registers Testing Library's `afterEach(cleanup)`
  (Vitest runs with `globals: false`). Tests do not need their own cleanup.
- The webview CSP allows inline `style` attributes and no inline scripts.

**The tarpack registry entry.** Add `{ id: "tarpack", label: "Tar Packager",
view: TarpackView }`, where `TarpackView` is a placeholder in
`src/tools/tarpack/TarpackView.tsx` showing the heading "Tar Packager". U2
replaces the body.

### Tokens to create

Create these in `apps/desktop/src/styles/tokens.css`:

- light values on `:root`;
- dark values under `@media (prefers-color-scheme: dark)` on
  `:root:not([data-theme="light"])`;
- dark values again under `:root[data-theme="dark"]`, as a hook for a possible
  later manual toggle.

**Theme (decided).** The app follows the system theme only. Build no toggle
and no theme setting; the `data-theme` selectors are just a dormant hook.

| Token | Light | Dark |
| --- | --- | --- |
| `--bg` | `#f7f7f8` | `#16171a` |
| `--surface` | `#ffffff` | `#1e2024` |
| `--surface-sunken` | `#eef0f2` | `#121316` |
| `--border` | `#d9dce1` | `#33363d` |
| `--text` | `#1b1d21` | `#e8e9ec` |
| `--text-muted` | `#5b616b` | `#a3a8b1` |
| `--accent` | `#2457c5` | `#7aa2ff` |
| `--accent-text` | `#ffffff` | `#0d1530` |
| `--ok` | `#1d7a46` | `#5fd08f` |
| `--warn` | `#8a5a00` | `#f2c14e` |
| `--danger` | `#b3261e` | `#ff8a80` |
| `--focus-ring` | `#2457c5` | `#7aa2ff` |
| `--drop-overlay` | `rgb(36 87 197 / 0.08)` | `rgb(122 162 255 / 0.12)` |

Other tokens:

- **Spacing:** `--space-1` to `--space-6` = 4, 8, 12, 16, 24, 32 px.
- **Radius:** `--radius` = 6 px.
- **UI font:** `--font-ui` = `"Segoe UI", system-ui, sans-serif`.
- **Monospace font:** `--font-mono` = `"Cascadia Mono", Consolas, monospace`.
  Use it for paths and modes.

`apps/desktop/src/styles/base.css` sets:

- the box-sizing reset;
- `body` background `--bg`, color `--text`, font `--font-ui`;
- a visible `:focus-visible` outline of 2 px in `--focus-ring`.

### Components

| Component | Path | Props | Notes |
| --- | --- | --- | --- |
| `AppShell` | `src/app/AppShell.tsx` | `tools` | sidebar plus main area; renders the active tool's `view` |
| `ToolNav` | `src/app/ToolNav.tsx` | `tools, activeId, onSelect` | `nav` landmark labelled "Tools" |

**Rules that bind this task.**

- Components use tokens only; there is no hex anywhere outside `tokens.css`.
- Every interactive element is keyboard reachable and labelled, with a visible
  focus state.
- Nothing is conveyed by color alone.
- The layout survives 200% text size.

## Files

- `apps/desktop/src/styles/tokens.css`, `apps/desktop/src/styles/base.css`
- `apps/desktop/src/app/AppShell.tsx`, `apps/desktop/src/app/ToolNav.tsx`
- `apps/desktop/src/App.tsx`, `apps/desktop/src/main.tsx`: import the styles
- `apps/desktop/src/tools/registry.ts`: add the entry
- `apps/desktop/src/tools/tarpack/TarpackView.tsx`: the placeholder
- Tests next to the components

Add dev dependencies `@testing-library/react` and `vitest-axe` (or
`jest-axe`) if they are absent.

## Skill

`impeccable:impeccable` with `craft`, then `impeccable:colorize` to confirm the
palette.

## Acceptance criteria

- Light and dark both render entirely from tokens; a grep for `#[0-9a-f]{3,6}`
  under `src/` finds only `tokens.css`.
- Every text/background pair in the table meets WCAG AA:
  - `--text` and `--text-muted` on `--bg`, `--surface`, and `--surface-sunken`;
  - `--accent-text` on `--accent`;
  - `--ok`, `--warn`, and `--danger` on `--surface`.
- `ToolNav` is a `nav` landmark. The active item has `aria-current="page"`.
  Up/Down arrows move between items, and Enter or Space selects.
- At 800×560 there is no horizontal scroll, and the main area fills the
  remaining height.

## Tests proving completion

`npm run test`:

- `AppShell.test.tsx`: renders the registry entries, and selecting a tool
  renders its view.
- `ToolNav.test.tsx`: arrow-key navigation and `aria-current`.
- An axe check on `AppShell` with no violations.
- `tokens.test.ts`: parses `tokens.css` and asserts the contrast pairs above
  meet 4.5:1, using a small WCAG luminance helper in the test.

## States covered

The single tool is selected. The navigation state exists for one or more tools.

## Out of scope

- Any tarpack content beyond the placeholder heading.
- A manual theme toggle (decided: the theme follows the system only).
