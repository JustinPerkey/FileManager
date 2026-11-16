# Product

<!-- impeccable:product-schema 1 -->

Written 2026-09-29 by the `impeccable` init flow from the approved plans in
`docs/plans/project/` and `CLAUDE.md`. The human confirmed the record in one
answer round. Facts marked *(inferred)* come from the repository, not from an
answer.

## Platform

web

FileManager ships as a Tauri v2 desktop app on Windows, but its interface is a
React page in the system webview (WebView2). Design and audit it as a web
surface with a desktop viewport: it has no touch target, no mobile layout, and
no native control vocabulary of its own.

## Users

- **Primary:** a developer or tester who rebuilds the same deployment package
  many times a day for a low-power armv7 Linux target. They know the file set.
  They want to confirm that everything is present and correct, pick the
  archive format, and build. The extraction command and Edit in editor are
  behind a developer mode (Ctrl+Alt+Shift+D) for the people who maintain
  manifests.
- They use the app at a desk, alongside an editor and a terminal, in short
  repeated visits. The next visit is minutes away. *(inferred)*
- **Database Manager users:** the same people, when they keep the data in a
  SQLite file correct: fixing a value, adding a row, linking two objects, or
  removing a stale one. They know the data, and do not want to write SQL or
  read foreign-key integers in a grid. An administrator or maintainer writes
  the one app-wide TOML config that decides what is editable. A developer
  edits it from the app in developer mode (Ctrl+Alt+Shift+D, "Edit
  config"). *(inferred from the Database Manager plan,
  2026-11-16)*

## Product Purpose

FileManager is a portable Windows desktop app that hosts several independent
file tools: the **Pipeline**, the **Tar Packager**, the **Schedule Creator**
and the **Database Manager**. The first was the **Tar Packager** (`tarpack`). It builds a Linux
archive from Windows files, as listed in a TOML manifest, with the exact Linux
paths, modes, owners, and line-ending treatment the target needs.

Success is an archive that extracts correctly on the target on the first try,
with nothing missing and no wrong permissions. The user should never need to
open the archive to check it.

## Positioning

A generic archiver on Windows cannot show Linux permissions, owners, or
absolute target paths before it writes them. The Tar Packager makes these the
subject of the screen: one row per manifest file, each with an explicit status,
and a build that verifies the archive before anything replaces the output.

The **Database Manager** (`dbmanager`) shows the objects in a SQLite file
through an object editor rather than a database grid, for people who would
otherwise need SQL or a generic browser that shows raw ids:

- each object is a form of typed fields;
- references are the referenced object's label, a link you can follow and
  come back from;
- many-to-many relations are checklists of every object on the other side;
- a delete lists everything it takes with it before it happens.

## Operating Context

- The manifest is a TOML file that the user edits in their own editor. The
  app reloads it, and warns when it changed on disk.
- Files come from scattered build folders. The user assigns them by dropping
  files or folders on the window, or by browsing for one row.
- On the target, GNU tar extracts with `-P`, because stored paths are
  absolute. The app shows the exact command and copies it.
- Formats: `.tar`, `.tar.gz`, `.tar.zst`, `.tar.xz`.

## Capabilities and Constraints

- Windows only. It ships as a portable `.exe` with no installer.
- The UI is a webview under a restrictive CSP: no remote fonts, images, or
  scripts; no inline scripts. Assets ship with the frontend.
- All filesystem work happens in Rust. The UI calls typed wrappers in
  `apps/desktop/src/lib/` and renders the session the backend returns.
- Destructive operations are confirmed or recoverable. A partial batch failure
  is reported, not swallowed.
- Paths are OS paths. The UI receives display strings, which may contain
  U+FFFD where a name is not valid UTF-8.
- The second tool is the **Schedule Creator** (`schedule`): it reads a
  schedule from a text file and adds it to an existing XML file, both
  specified by the user (typed path, Browse, or drop), after a confirmation
  and a backup. Its parsing and XML update are not written yet;
  the UI says so.
- The third tool, and the first in the navigation, is the **Pipeline**
  (`pipeline`): the user chooses one folder, and four steps run over its
  subfolders in order: check required files, run each subfolder's batch file,
  the schedule step, and tar packing. The user goes step by step ("Run
  next"; each step unlocks once the one before it passes) or runs them all at
  once (stopping at the first that does not pass); the ones that change
  things ask first. The steps themselves are not written yet; the UI says so,
  and until the first one is, the later steps stay locked.
- The fourth tool is the **Database Manager** (`dbmanager`).
  - **Opening:** the user opens a SQLite file (never created by the app), by
    Open, Recent databases, or dropping it on the window.
  - **The config:** one app-wide TOML config at
    `%ProgramData%\FileManager\dbmanager.toml`.
    - The user never chooses it, and the app only reads it and reloads it
      automatically when it changes.
    - It marks tables (editable, read-only, hidden, or pickable lookup
      tables), fields and many-to-many relations, and sets field bounds.
  - **Defaults:** anything the config does not mention is editable. With no
    config file, everything is editable, and a calm banner says so (no
    prompt). A config the app cannot use makes everything read-only until it
    is fixed.
  - **Values:** booleans are checkboxes; durations are typed as `1h30m`;
    there are no dates.
  - **Writes:** saves are explicit and transactional. The first write of each
    session is preceded by a backup beside the database. Deleting is opt-in
    per table, and always shows its impact first. Closing the window with
    unsaved edits asks first.

## Brand Commitments

- The name is **FileManager**. The tools are the **Pipeline**, the **Tar
  Packager**, the **Schedule Creator**, and the **Database Manager**.
- Voice: plain, short, and exact. Controls name their action. Errors name the
  problem and the recovery.
- The theme follows the system. No manual toggle (decided 2026-09-29).

## Evidence on Hand

- The manifest format is documented in `docs/tarpack-manifest.md` (once M3
  lands).
- The Database Manager's config format is documented in
  `docs/dbmanager-config.md`, with an example schema and config in
  `examples/dbmanager/`.
- There are no screenshots, testimonials, or usage metrics, and none should be
  invented.

## Product Principles

1. **Show what will be written.** Put the Linux path, mode, owner, and line
   endings on screen before the build, not after.
2. **Never guess.** An ambiguous match or a changed manifest stops and asks.
   It is never resolved silently.
3. **The loop is the product.** Reload, check, build, and copy take a few
   keystrokes, with no mouse trip.
4. **Nothing destructive by accident.** Overwrites are confirmed. A failed
   verification leaves the old file untouched, and the copy says so.

## Accessibility & Inclusion

- WCAG 2.2 AA is a requirement, not polish. Every interactive element is
  keyboard reachable and labelled, with a visible focus state.
- Nothing is conveyed by color or icon alone.
- The layout holds at 200% text size and at the 800×560 minimum window.
- Motion respects `prefers-reduced-motion`.
