# M7 — Portable Windows exe and end-to-end check

Status: awaiting approval
(amended 2026-09-30: step 10, the WebView2 middle-truncation check, moved
here from U6's review; the Linux steps are now 11 and 12)
Project: tarpack   Depends on: M6 (landed), U6 (landed)

## Goal

Produce a portable Windows `.exe` that a user can run without installing
anything, publish it from CI, and prove the whole Tar Packager flow end to end,
from Windows to extraction on the Linux target.

## Context

- The app is a Tauri v2 shell at `apps/desktop/src-tauri`. Its settings are in
  `tauri.conf.json`. `npm run tauri:build` already runs `tauri build
  --no-bundle` (from M1), and the exe lands at `target/release/filemanager.exe`.
- CI is `.github/workflows/ci.yml`. Its `windows-latest` job already builds
  the frontend and runs `cargo test --workspace --locked` and then
  `npm run tauri:build` on the MSVC toolchain.
- `tauri.conf.json` already has `bundle.active: false` with **no
  `bundle.targets`**, and a restrictive CSP in `app.security` (from M1). Do not
  add bundle targets. Do not loosen the CSP.
- **Distribution is a portable `.exe`, with no installer** (the human's
  decision). There is no NSIS or MSI bundle.
- **The exe links C code.** `fm-tarpack` depends on `zstd` (bundled libzstd)
  and `liblzma` (or `xz2`; bundled liblzma), both compiled by the `cc` crate
  with MSVC. `flate2` is pure Rust.
- **Static C runtime.** By default MSVC binaries import `vcruntime140.dll`,
  which a clean machine may lack, and a portable exe cannot install the VC++
  redistributable. Link the CRT statically for
  `x86_64-pc-windows-msvc`. `.cargo/config.toml` already exists from M1, with
  an `[env]` section setting `TS_RS_EXPORT_DIR`. Keep that section, and add:

  ```toml
  [target.x86_64-pc-windows-msvc]
  rustflags = ["-C", "target-feature=+crt-static"]
  ```

  The `cc` crate reads this target feature and compiles the zstd and liblzma
  sources with `/MT` to match. Mismatched CRTs fail at link time with LNK2038;
  if that happens, fix it, do not suppress it.
- **WebView2.** The exe uses the Evergreen WebView2 runtime that ships with
  Windows 11 and current Windows 10. A portable exe cannot bootstrap it
  (`bundle.windows.webviewInstallMode` applies only to installers, so leave it
  alone). Document the requirement in the README, with the Microsoft download link, for machines
  that lack it.
- **State location.** App state stays in `%APPDATA%\FileManager\` and manifests
  default to `%APPDATA%\FileManager\tarpack\manifests\`. "Portable" means no
  installer, not data beside the exe. Do not change this.
- Code signing is out of scope. The unsigned exe shows a SmartScreen prompt.
  Document that in the README.
- **The archive** stores absolute names (`/opt/...`). The target is an armv7
  device with GNU tar and zstd. The extraction commands for all four output
  formats (`.tar`, `.tar.gz`, `.tar.zst`, `.tar.xz`) are documented in
  `docs/tarpack-manifest.md`. Every one uses `-P`, which is required, and
  `--no-overwrite-dir`. The app shows the exact command in the build result.
- The example manifest is `examples/tarpack/example.toml`.
- **Middle-truncated paths (from U6).** The Tar Packager view shows long
  Windows paths on one line, truncated in the middle by CSS, in the component
  `apps/desktop/src/tools/tarpack/MiddlePath.tsx`. It is used in the entry
  table's Windows location column (`EntryTable.tsx`) and for the build bar's
  output path. The rendered result is "C:\Users\…\name.ext": the start of
  the path, "…", then the whole file name with its leading `\`. The line
  snaps to whole characters with `width: calc(round(down, 100% - 1px, 1ch) +
  0.5px)`, and the tail is `flex: 0 0 auto; max-width: calc(100% - 4ch)`.
  This was verified only in Chromium on Linux with a fallback monospace font.
  WebView2 on Windows renders with Cascadia Mono or Consolas, so the check in
  step 10 is its first test with the real fonts. The full path is shown by
  design on hover (`title`) and on the keyboard-focused table row, so those
  are not truncation failures.
- **The table's two layouts.** Below a table width of 64rem each row is
  stacked (the default 1000×700 window shows this); at 64rem or wider the
  table shows columns, which at 100% text needs a window about 1282 px wide
  or wider. A maximised window on a 1920 px wide screen shows the column
  layout.

**Rules that bind this task.**

- No secrets in the repository or the workflow. Signing keys, if they are ever
  added, come from CI secrets and are out of scope here.
- The end-to-end check uses throwaway files in a scratch folder, never real
  user data.
- **Extraction with `-P` writes to the real `/`.** In the check, it runs only
  on a disposable target: a throwaway container or VM, or a test device meant
  for deployment. It never runs on a developer's own Linux machine.

## Files

- `.cargo/config.toml`: add the static-CRT target section (above), keeping
  the existing `[env]` section
- `apps/desktop/src-tauri/tauri.conf.json`: product name `FileManager`,
  identifier, version, and the icons embedded in the exe
- `.github/workflows/ci.yml`, in the Windows job, after `tauri:build`:
  - Run `dumpbin /dependents target\release\filemanager.exe` (from a VS
    developer shell, for example via `ilammy/msvc-dev-cmd`). Fail the job if
    the output lists `vcruntime*.dll`, `msvcp*.dll`, `api-ms-win-crt-*`,
    `zstd*.dll`, or `liblzma*.dll`.
  - Upload the exe as an artifact named `FileManager-<version>-x64.exe`,
    without the `.pdb`.
- `docs/tarpack-e2e.md`: the manual checklist below, with a results table
- `README.md`:
  - how to run the portable exe;
  - the WebView2 requirement;
  - the SmartScreen prompt;
  - where state lives;
  - how to extract each format on the target, pointing at
    `docs/tarpack-manifest.md` for the full command table.
- `CLAUDE.md`, Commands: note the static CRT setting and the dependency check.

## The end-to-end checklist

Record each step with pass/fail and notes in `docs/tarpack-e2e.md`.

1. Copy the CI artifact exe to a clean Windows machine (no Rust, no Visual
   Studio, no VC++ redistributable) and run it. It starts, with only the
   SmartScreen prompt.
2. Open `examples/tarpack/example.toml`, copied to a scratch folder. It shows
   no errors, and the entry with `normalize_eol` is marked.
3. Drop a folder containing some of the listed files. The matched,
   unmatched, and ambiguous results are correct.
4. Assign one remaining file with Browse. Give the `normalize_eol` entry a
   file saved with CRLF line endings.
5. The default format follows the example's `output_name` (`.tar.zst`). Choose
   an output path and create the archive. The summary shows:
   - the entry count, the on-disk and uncompressed sizes, and the SHA-256;
   - the extraction command with `-P`;
   - the normalised entry, with its CRLF count.
   Press both Copy buttons and paste into Notepad: the hash and the command
   arrive exactly. The UI uses `navigator.clipboard.writeText` with no
   clipboard plugin. If Copy fails in the packaged exe, record the failure and
   report it. The fix is a separate backend task (a clipboard plugin wrapper),
   not part of this task.
6. Build again to the same path. The overwrite confirmation appears, and
   Cancel leaves the old file untouched.
7. Switch the format to each of `.tar`, `.tar.gz`, and `.tar.xz`, and build
   each. The Save name and output path extension follow the format every time.
8. Close and reopen the app. The manifest, all sources, and the last chosen
   format are restored. A deleted source shows as Missing.
9. Edit the manifest in an external editor (change a mode). The banner
   appears. Building before reload is refused. After reload, the new mode is
   shown.
10. **WebView2 middle truncation** (the user runs this on the Windows
    machine, in the packaged exe; it moved here from U6 because it needs
    Windows). Set up long paths first: assign the sources from a scratch
    folder nested deep enough that the Windows location column truncates,
    and choose an output path in a similarly deep folder, so the build bar's
    output path truncates too. Record the Windows version and, if you
    can tell, which monospace font rendered (Cascadia Mono or Consolas). Then, for every
    truncated path, in the table and in the build bar's output path:
    - "…" sits directly against the file name's leading `\`, with no gap;
    - the file name is never split mid-word.

    Check this at several window widths, resizing a pixel or two at a time
    over a range of at least 20 px each time, in **both table layouts**
    (stacked at the default window, columns when maximised), and again at
    **200% text**: set Windows Settings > Accessibility > Text size to 200%,
    and restart the app. If the app's text
    does not grow with that setting, record "200% text: not applied by
    WebView2" in the notes instead of passing or failing it, and report it;
    do not change the app's zoom settings to force it. Do not keyboard-focus
    a table row or hover a path while checking, since both show the full path
    by design. Restore the text size afterwards.

    **If it fails:** record the failure in the results table with the window
    width (and text size and layout) where it shows, and a screenshot. Keep
    the screenshot outside the repository and give its path, or attach it to
    the report. Do not change `MiddlePath` or any other UI file in this task.
    A failure becomes a follow-up UI task for the ui-implementer, reported to
    the orchestrator; it does not block the rest of the checklist.
11. **On a disposable Linux target**, for each of the four files:
    - Run the listing command, `tar -tvPf` plus the format flag. Every name
      begins with `/`, and the modes and owners match the manifest.
    - Run the exact extraction command shown by the app, as root. Check that:
      - the files land at their absolute paths, with the manifest's modes and
        owners;
      - newly created parent directories have `dir_mode`;
      - a pre-existing directory (for example `/etc`) keeps its original mode
        and owner;
      - the normalised script has no CR bytes (`grep -c $'\r'` gives `0`) and
        runs.
12. **Decompression cost on armv7.** On the armv7 target, or failing that an
    armv7 container under qemu (note which), run
    `/usr/bin/time -v tar --zstd -xpPf …` and the `.tar.xz` equivalent. Record
    the peak resident set size of each. Expect roughly 8 MiB (zstd) and 9 MiB
    (xz) above tar's own baseline. Record the wall time too.

## Acceptance criteria

- CI uploads the portable exe as an artifact on every push to the branch.
- The CI dependency check proves that the exe imports no C runtime, zstd, or
  liblzma DLL.
- `docs/tarpack-e2e.md` records all 12 steps, with the commit SHA tested.
  Steps 1–9, 11, and 12 pass. Step 10 has been run on Windows and is
  recorded: pass, or fail with the window width, the layout, the text size,
  and a screenshot path, and a follow-up UI task reported. M7 is not signed
  off until step 10 has been run.
- The README documents running the portable exe, WebView2, the SmartScreen
  prompt, the state location, and extraction with `-P` for each format.

## Tests proving completion

- The Windows CI job is green, including the `dumpbin` check, and its artifact
  is present.
- The checklist is recorded in `docs/tarpack-e2e.md`.

## Out of scope

- Code signing.
- Auto-update.
- An installer, or bundling a fixed-version WebView2 runtime.
- New features found missing during the check. Report those; do not build them.
- Fixing a step 10 (middle truncation) failure. It is a UI change, owned by
  the ui-implementer through a follow-up UI task.

## Risks

- `+crt-static` applies to every crate built for the target, including tests.
  Confirm that `cargo test --workspace` still passes on the Windows job after
  adding it.
- The CSP (from M1) is `default-src 'self'` with IPC allowed and inline
  styles allowed. If a view works under `tauri:dev` but misbehaves only in the
  packaged exe, suspect the production CSP. Report the blocked directive; do
  not remove or loosen the policy in this task.
- Artifact size: exclude debug symbols from the uploaded artifact. Consider
  `strip = true` and `lto = true` in the release profile if the size is a
  problem. Do not change opt-level without measuring.
- A missing WebView2 runtime makes the exe fail at startup. Only the README
  covers this; there is no in-app recovery.
