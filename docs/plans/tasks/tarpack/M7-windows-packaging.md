# M7 — Windows packaging and end-to-end check

Status: awaiting approval (the bundle format waits on the human's answer about
installer or portable exe)
Project: tarpack   Depends on: M6 (landed), U6 (landed)

## Goal

Produce a Windows build a user can run, publish it from CI, and prove the
whole Tar Packager flow end to end on Windows.

## Context

- The app is a Tauri v2 shell at `apps/desktop/src-tauri`. Its bundle settings
  are in `tauri.conf.json`.
- CI is `.github/workflows/ci.yml`. Its `windows-latest` job already runs
  `npm run tauri:build`.
- The bundle format is **`<to be filled in by the planner once the human
  answers: nsis | msi | portable exe>`**. Do not start this task while this
  line is unresolved; report it as a blocker.
- Code signing is out of scope. The unsigned build shows a SmartScreen prompt.
  Document that in the README.
- The manifest format is documented in `docs/tarpack-manifest.md`, and the
  example manifest is `examples/tarpack/example.toml`.

**Rules that bind this task.**

- No secrets in the repository or the workflow. Signing keys, if they are ever
  added, come from CI secrets and are out of scope here.
- The end-to-end check uses throwaway files in a scratch folder, never real
  user data.

## Files

- `apps/desktop/src-tauri/tauri.conf.json`: bundle targets, product name
  `FileManager`, identifier, version, icons
- `.github/workflows/ci.yml`: upload the Windows bundle as a build artifact
- `docs/tarpack-e2e.md`: the manual checklist below, with a results table
- `README.md`: how to install, run, and extract the produced tar on Linux

## The end-to-end checklist

Record each step with pass/fail and notes in `docs/tarpack-e2e.md`.

1. Install or run the CI artifact on a clean Windows machine.
2. Open `examples/tarpack/example.toml`, copied to a scratch folder. It shows
   no errors.
3. Drop a folder containing some of the listed files. The matched,
   unmatched, and ambiguous results are correct.
4. Assign one remaining file with Browse.
5. Choose an output path and create the tar. The summary shows the entry count,
   size, and SHA-256.
6. Build again to the same path. The overwrite confirmation appears, and
   Cancel leaves the old file untouched.
7. Close and reopen the app. The manifest and all sources are restored; a
   deleted source shows as Missing.
8. Edit the manifest in an external editor (change a mode). The banner
   appears. Building before reload is refused. After reload, the new mode is
   shown.
9. On Linux, run `tar -tvf` and then `tar -xpf <file> -C <tmp>`. The paths,
   modes, and owners match the manifest, and parent directories have
   `dir_mode`.

## Acceptance criteria

- CI uploads a Windows bundle artifact on every push to the branch.
- `docs/tarpack-e2e.md` records all 9 steps passing, with the commit SHA
  tested.
- The README documents install, the SmartScreen prompt, and Linux extraction.

## Tests proving completion

- The Windows CI job is green, and its artifact is present.
- The checklist is recorded in `docs/tarpack-e2e.md`.

## Out of scope

- Code signing.
- Auto-update.
- New features found missing during the check. Report those; do not build them.

## Risks

- Artifact size: exclude debug symbols from the uploaded artifact.
