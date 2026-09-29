---
name: FileManager
description: A quiet, dense Windows desktop utility whose tools show exactly what they will write before they write it.
colors:
  bg: "#f7f7f8"
  surface: "#ffffff"
  surface-sunken: "#eef0f2"
  border: "#d9dce1"
  text: "#1b1d21"
  text-muted: "#5b616b"
  accent: "#2457c5"
  accent-text: "#ffffff"
  ok: "#1d7a46"
  warn: "#8a5a00"
  danger: "#b3261e"
  focus-ring: "#2457c5"
  bg-dark: "#16171a"
  surface-dark: "#1e2024"
  surface-sunken-dark: "#121316"
  border-dark: "#33363d"
  text-dark: "#e8e9ec"
  text-muted-dark: "#a3a8b1"
  accent-dark: "#7aa2ff"
  accent-text-dark: "#0d1530"
  ok-dark: "#5fd08f"
  warn-dark: "#f2c14e"
  danger-dark: "#ff8a80"
  focus-ring-dark: "#7aa2ff"
typography:
  title:
    fontFamily: "Segoe UI, system-ui, sans-serif"
    fontSize: "1.25rem"
    fontWeight: 600
    lineHeight: 1.25
  body:
    fontFamily: "Segoe UI, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.4
  mono:
    fontFamily: "Cascadia Mono, Consolas, monospace"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.4
rounded:
  md: "6px"
spacing:
  "1": "4px"
  "2": "8px"
  "3": "12px"
  "4": "16px"
  "5": "24px"
  "6": "32px"
components:
  tool-nav-item:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: "8px 12px"
  tool-nav-item-current:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: "8px 12px"
---

# Design System: FileManager

Generated 2026-09-29 by the `impeccable` document flow, in scan mode, from
`apps/desktop/src/styles/` as landed in U1. The normative source is
`apps/desktop/src/styles/tokens.css`; this file describes it and must change
with it. Sections marked **Planned (U2)** describe vocabulary that the approved
UI plan adds in task U2. They are rules for that task, not yet code.

## Overview

**Creative North Star: "The Packing List"**

The interface is a checked packing list for a crate that is about to ship.
Each line says what goes in, where it will land, and whether it is ready. The
system is quiet so that the list can be loud: neutral surfaces, one blue
accent for the action and the current place, and status colors reserved for
status. It is dense by design, because the user comes back many times a day
and wants the whole picture in one viewport.

Depth comes from tone, not shadow. The sidebar sits one step darker than the
content, panels are separated by 1 px borders, and nothing floats unless it
has to: dialogs and the drop overlay.

**Key Characteristics:**

- A restrained palette: neutrals, one accent, and three status hues.
- System UI type (Segoe UI), with Cascadia Mono for data: paths, modes,
  hashes, and commands.
- A fixed rem scale, with no fluid type.
- Flat, tonal layering, with 1 px borders and a 6 px radius.
- The theme follows the system: light and dark are both first-class.

## Colors

A cool neutral ground with a single working blue. Color means something here,
or it is absent.

### Primary

- **Working Blue** (`#2457c5` light / `#7aa2ff` dark): the primary action
  (**Create archive**), the current tool marker, the focus ring, and the drop
  overlay's border. Never decoration.

### Neutral

- **Paper Grey** (`--bg` `#f7f7f8` / `#16171a`): the window ground.
- **Sheet White** (`--surface` `#ffffff` / `#1e2024`): content panels, table,
  and build bar.
- **Tray Grey** (`--surface-sunken` `#eef0f2` / `#121316`): the sidebar, code
  and command blocks, and the CRLF → LF marker. It is the second neutral layer.
- **Rule Grey** (`--border` `#d9dce1` / `#33363d`): 1 px dividers and control
  outlines.
- **Ink** (`--text` `#1b1d21` / `#e8e9ec`) and **Pencil** (`--text-muted`
  `#5b616b` / `#a3a8b1`): body text and secondary text. Both pass AA on all
  three surfaces (asserted by `tokens.test.ts`).

### Status

- **Ready Green** (`--ok` `#1d7a46` / `#5fd08f`), **Caution Amber** (`--warn`
  `#8a5a00` / `#f2c14e`), and **Stop Red** (`--danger` `#b3261e` / `#ff8a80`).
  Each passes AA on `--surface`. They are always paired with an icon and a
  word.

### Named Rules

**The Status Is Earned Rule.** `--ok`, `--warn`, and `--danger` appear only
on status: row status, banners, and results. They never appear on
decoration, headings, or hover.

**The One Blue Rule.** The accent marks one primary action per region, the
current place, and focus. A second blue button in the same region means one
of them is not primary.

## Typography

**Body Font:** Segoe UI (with `system-ui`, then `sans-serif`)
**Mono Font:** Cascadia Mono (with Consolas, then `monospace`)

**Character:** the platform's own voice, so the tool feels native to Windows,
plus a coding mono for anything the user might paste into a terminal.

### Hierarchy

- **Title** (600, 1.25rem): the view heading, for example the manifest name.
- **Body** (400, 0.875rem, line-height 1.4): everything else. Set on `body`.
- **Mono** (400, 0.875rem): paths, modes, hashes, and commands. Monospace is
  for data, never a "technical" costume.

**Planned (U2).** Four size tokens fix the scale, so no component invents a
size: `--font-size-sm` 0.8125rem, `--font-size-md` 0.875rem (body),
`--font-size-lg` 1rem, and `--font-size-xl` 1.25rem (title). Counts, sizes,
percentages, and octal modes use `font-variant-numeric: tabular-nums`.

### Named Rules

**The rem Rule.** Sizes are in rem so that the 200% text setting scales
everything. There is no `clamp()` and no viewport-based type.

## Layout

A fixed two-column desktop frame. A 12rem sidebar (capped at 40% of the
window) sits beside a main area that fills the rest and gives the active
tool's view its full height. Each view owns its own scrolling. The Tar
Packager view stacks, from top to bottom: the header, the problems region,
the entry table (the only region that grows), and a sticky build bar. The
minimum window is 800×560. At that size, and at 200% text, nothing scrolls
horizontally at page level; toolbars wrap onto a second line.

Spacing follows the `--space-1…6` scale (4, 8, 12, 16, 24, 32 px): tight
inside groups, with a wider gap between them. The view is padded
`--space-4 --space-5`.

## Elevation & Depth

Flat. Depth is tonal: `--surface-sunken` is below and `--surface` is above,
separated by 1 px `--border`. No component has a shadow at rest. Only the
confirmation dialog and the shortcuts popover float, with one soft, offset
shadow (`0 8px 24px rgb(0 0 0 / 0.18)`, planned in U5), over a dimmed
backdrop for the dialog.

## Shapes

One radius, `--radius` 6 px, on controls, panels, the marker, and the tool
nav items. Borders are 1 px. The only thicker line is the 2 px dashed drop
overlay border, and the 2 px focus outline with a 2 px offset.

## Components

### Navigation (landed, U1)

- **Style:** full-width text buttons in the Tray Grey sidebar, with 8×12 px
  padding.
- **Current:** a `--surface` fill, a 1 px `--border` outline, and weight 600,
  with `aria-current="page"`.
- **Known drift:** the current item also has a 3 px `--accent` left border.
  This is the thick colored side stripe the craft floor refuses. U6 replaces
  it with a 1 px outline, and optionally a small accent dot or icon. Weight
  and fill already carry the state.

### Buttons (Planned, U2)

- **Shape:** `--radius`, 1 px border, `--font-size-md`, at least 24×24 px, and
  padding `--space-1 --space-3`.
- **Primary:** `--accent` fill, `--accent-text` label. There is one per region
  (for example, **Create archive**).
- **Secondary:** `--surface` fill, `--border` outline, `--text` label. This is
  the default.
- **Quiet:** transparent, with no border until hover. Use it for row actions
  and dismiss.
- **States:** hover, active, focus-visible (the global 2 px ring), disabled
  (`--text-muted` on `--surface-sunken`, no pointer), and busy
  (`aria-busy="true"`, label unchanged).
- **Menu trigger:** a chevron icon, never the `▾` glyph.

### Icons (Planned, U2)

One authored inline-SVG set in `src/app/icons.tsx`: a 16 px grid, 1.5 px
stroke, round joins, and `currentColor`. It is `aria-hidden` unless it is the
only content, which is never the case in this app. There are no Unicode or
emoji stand-ins.

### Banners (Planned, U2)

`--surface` fill with a 1 px border in the tone color, a tone icon, and the
message in `--text`. The tone is also stated in the accessible name. There is
no tinted fill and no thick left stripe.

### Data table (Planned, U3)

The table is the product's signature component. It uses the body size, sticky
headers on `--surface`, 1 px row rules, and mono for path, target, and mode.
Row hover is `--surface-sunken`. Each row status is an icon plus a word in its
status color.

## Do's and Don'ts

### Do:

- **Do** take every color from `tokens.css`. The only hex in `src/` lives
  there.
- **Do** pair every status color with an icon and a word.
- **Do** middle-truncate Windows paths, keeping the drive and the file name,
  with the full path in `title` and in visually hidden text.
- **Do** wrap, never truncate, messages the user must read in full: the
  extraction command, the SHA-256, and warning text.
- **Do** theme browser surfaces from tokens: `::selection`, `scrollbar-color`,
  and tabular numerals.

### Don't:

- **Don't** use a colored side border thicker than 1 px on nav items, rows,
  banners, or callouts.
- **Don't** add shadows, gradients, or glass to panels at rest.
- **Don't** use Unicode glyphs (`▾`, `✓`, `⚠`) as icons.
- **Don't** open a modal for anything but the overwrite confirmation.
- **Don't** animate for decoration. Motion shows state in 120–200 ms, and not
  at all under `prefers-reduced-motion`.
