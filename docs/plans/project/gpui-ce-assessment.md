# Assessment — migrating the UI from Tauri + React to GPUI CE

Status: assessment only (2026-11-19). No plan or milestones until the human
decides. Facts about GPUI CE are from its README, CHANGELOG, MAINTENANCE.md,
issue tracker, crates.io API and docs.rs guides as fetched on 2026-11-19
(version 0.3.0); anything not found there is marked **unconfirmed**.

## Recommendation

**No-go for now.** Stay on Tauri v2 + React. Revisit when the triggers at the
end are met. If the human wants more certainty first, run the time-boxed spike
below; it answers every unconfirmed point for a small fraction of the
migration's cost.

## What would change

| Area | Today | With GPUI CE |
|---|---|---|
| UI code | ~4,450 lines TS/TSX in 82 files (3 tool views, shell, shared controls) | Rewritten in Rust |
| UI tests | ~4,500 lines, 395 vitest + Testing Library + axe tests | Rewritten with `#[gpui::test]` / `TestApp`; no axe equivalent |
| Styles | ~1,600 lines CSS; `styles/tokens.css` is normative for `DESIGN.md` | Re-expressed as Rust style code; token system rebuilt |
| Boundary | 55 ts-rs generated types, `lib/*.ts` clients, Tauri commands/events | Removed: UI calls the domain crates directly |
| Shell | ~4,600 lines Rust in `src-tauri` (commands, state, watchers, tests) | Command layer removed; session logic (`core.rs` files) largely reusable |
| Domain crates | `fm-core`, `fm-tarpack`, `fm-schedule`, `fm-pipeline`, `fm-dbmanager` | Unchanged (none depends on a UI) |
| Tooling | `impeccable` skill, jsx-a11y + oxlint, axe, `screenshots/shoot.js` manual screenshots | All web-specific; no replacements identified |
| Runtime | WebView2 (bundled with Windows 11; present on most Windows 10) | Direct3D 11 feature level 11.0; no webview |

## Gains

- One language and one build; the ts-rs boundary, its currency test and the
  "change both sides" invariant disappear.
- No webview: no WebView2 dependency, no CSP to maintain, likely faster
  startup and lower memory.
- UI state can hold domain types directly (no string-encoded integers, no
  JSON mirroring), and backend validation could run inline, making M11 (the TS
  validation mirror) and the regex-dialect restriction from M5 unnecessary.

## Blocking risks

1. **Maturity and churn.** Pre-1.0; every minor release has breaking changes
   (0.3.0: `AppContext` split, text system moved to Parley, asset API and
   drag-drop reworked, renames). Two releases in the observed window; versions
   0.3.2/0.3.3 were yanked; semantic versioning is an open issue (#387). A
   single crates.io owner publishes it. No production app built on GPUI CE was
   found.
2. **Accessibility is a hard requirement here** (`CLAUDE.md`: every control
   keyboard reachable and labelled; axe checks per state). GPUI CE has an
   AccessKit tree, roles, ARIA-like properties, focus handles and Tab
   traversal, but:
   - Windows UI Automation support is **unconfirmed** (the platform page
     names AT-SPI for Linux only);
   - no high-contrast or text-scaling support was found; the app must pass
     reduced motion itself; there is no contrast checker;
   - open bug #282: an accessibility click on a scrolled-out element hits
     whatever is drawn there;
   - there is no automated equivalent of our axe checks.
3. **Missing widgets.** Core has layout primitives, virtual lists, popovers and
   text fields (`gpui_ce_elements`). Checkbox, table, dialog/modal and menu
   components are **not confirmed** in core. The rich component library
   (longbridge gpui-kit, ~75 components, AccessKit) targets **upstream Zed
   GPUI**, not GPUI CE, so it would need porting or verification. The Database
   Manager alone needs checkboxes (including the tri-state "not set"), paged
   lists, a picker overlay, confirmation dialogs and a checklist.
4. **Platform features we use today, unconfirmed on Windows:** drag-and-drop of
   files from Explorer (a reworked external-payload API exists), window-close
   interception (needed for U13/M14), static CRT and a single portable exe
   with no DLLs (our `dumpbin` CI check), minimum Windows version.
5. **Sunk and planned work.** The three shipped tools' UIs and their 395 tests
   would be rewritten; the Database Manager's 13 UI task plans, `DESIGN.md`
   and `PRODUCT.md` design work, and the manual's screenshot pipeline would be
   redone for a different toolkit; the agent chain's UI roles depend on the
   `impeccable` web skill.

## Options

| Option | What it means | Verdict |
|---|---|---|
| A. Stay on Tauri + React | Continue the current plan | **Recommended** |
| B. Full migration to GPUI CE | Rewrite every tool's UI; drop the shell's command layer | Not now (risks 1–4) |
| C. GPUI CE for the Database Manager only | Second UI stack beside Tauri | Rejected: two toolkits, two design systems, likely two executables, for the tool with the most UI |
| D. Upstream Zed GPUI + gpui-kit instead of GPUI CE | Larger community, richer components, production use (Zed, Longbridge Pro) | Worth comparing in the spike if a native Rust UI is still wanted; same accessibility and Windows questions apply |

## Spike, if wanted (time-boxed, throwaway, outside `main`)

One small GPUI CE window on Windows with: a labelled text field, a checkbox
(or `BaseSwitch`), a virtual list, a modal confirmation, an open-file dialog,
a file dropped from Explorer, close interception, and opening a file in the
system editor. Pass criteria:

- builds as one portable exe with `+crt-static` and passes our `dumpbin`
  check;
- Narrator and NVDA read every control's role, name and state; everything is
  reachable and operable by keyboard; Windows text scaling and high-contrast
  are honoured;
- Explorer drop delivers paths; close can be cancelled;
- a `#[gpui::test]` exercises the window headlessly in CI on Windows.

Repeat the same checklist against option D. A spike that passes everything
would justify a planner run for a staged migration (shell and shared controls
first, then one tool at a time, with the Database Manager UI built directly in
the new toolkit).

## Revisit triggers

- GPUI CE reaches 1.0, or a stable semver policy with a deprecation window.
- Windows UI Automation support is documented, and high-contrast/text scaling
  are supported.
- Checkbox, table and dialog components exist for GPUI CE (in core or a
  maintained library), or gpui-kit supports GPUI CE.
- At least one production Windows app ships on GPUI CE.

## Effect on the current plan

None if option A is chosen. Whatever is decided, M6–M9 (domain logic in
`fm-dbmanager`) are UI-independent and continue. Under a future migration,
M10's Tauri commands, M11's TypeScript mirror and M14's Tauri capability would
be replaced, and the U tasks re-planned.

## Addendum (2026-11-19): upstream Zed GPUI with gpui-kit

Asked whether regular (upstream) GPUI would be better. Sources: Zed's GPUI
README, crates.io API for `gpui` and `gpui-component`, longbridge/gpui-kit
README and Cargo.toml, zed.dev Windows pages, fetched 2026-11-19.

Better than GPUI CE on:
- **Windows maturity:** Zed itself has shipped stable on Windows since
  2025-10-15 (DirectX 11, DirectWrite), so the Windows backend carries a
  production editor. Nothing comparable exists for GPUI CE.
- **Components:** gpui-kit (Apache-2.0, ~21k stars, roughly weekly releases,
  latest `gpui-component` 0.10.0 on 2026-11-16) advertises 60–75+ components
  including virtual-scrolling data tables, virtual lists and forms, and powers
  a shipped commercial app (Longbridge Pro). This covers most of risk 3 above,
  pending a check of checkbox (incl. indeterminate), dialogs and menus.

Not better on, or still unknown:
- **Churn:** GPUI is pre-1.0 with "often breaking changes" and no promise to
  external users. The crates.io `gpui` release is a year old (0.2.2,
  2025-10-22); gpui-kit instead pins `gpui-pre` snapshots at an exact version
  (`=0.3.12`) because "any snapshot may change GPUI's API". Each gpui-kit
  upgrade is therefore a GPUI upgrade, weekly, from a single crates.io
  publisher.
- **Accessibility:** upstream GPUI and Zed publish no screen-reader
  documentation (their accessibility pages return 404), and crates.io `gpui`
  has no AccessKit feature. gpui-kit claims AccessKit roles, names, states and
  actions "covered by tests", but Windows UI Automation support is not stated.
  Risk 2 is unchanged and remains the deciding unknown.
- **Platform features:** native file dialogs, Explorer drag-and-drop and
  window-close interception were not verified for either.
- **Cost:** the rewrite described above is the same size.

Conclusion: if a native Rust UI is pursued, upstream GPUI + gpui-kit (option D)
is the candidate to spike, not GPUI CE. The recommendation stays **not now**:
the spike's accessibility checks (Narrator and NVDA on Windows, keyboard,
high contrast, text scaling) are the gate, followed by the portable-exe and
Explorer-drop checks.
