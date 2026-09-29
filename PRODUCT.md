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
  archive format, build, and copy the command that extracts it on the target.
- They use the app at a desk, alongside an editor and a terminal, in short
  repeated visits. The next visit is minutes away. *(inferred)*

## Product Purpose

FileManager is a portable Windows desktop app that hosts several independent
file tools. The first is the **Tar Packager** (`tarpack`). It builds a Linux
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
- Undecided: the tools that come after the Tar Packager.

## Brand Commitments

- The name is **FileManager**. The first tool is the **Tar Packager**.
- Voice: plain, short, and exact. Controls name their action. Errors name the
  problem and the recovery.
- The theme follows the system. No manual toggle (decided 2026-09-29).

## Evidence on Hand

- The manifest format is documented in `docs/tarpack-manifest.md` (once M3
  lands).
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
