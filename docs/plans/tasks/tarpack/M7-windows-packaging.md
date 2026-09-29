# M7 — Portable Windows exe and end-to-end check

Status: awaiting approval
Project: tarpack   Depends on: M6 (landed), U6 (landed)

## Goal

Produce a portable Windows `.exe` that a user can run without installing
anything, publish it from CI, and prove the whole Tar Packager flow end to end,
from Windows to extraction on the Linux target.

## Context

- The app is a Tauri v2 shell at `apps/desktop/src-tauri`. Its settings are in
  `tauri.conf.json`. `npm run tauri:build` already runs `tauri build
  --no-bundle` (from M1), and the exe lands at `target/release/filemanager.exe`.
- CI is `.github/workflows/ci.yml`. Its `windows-latest` job already runs
  `npm run tauri:build` on the MSVC toolchain.
- **Distribution is a portable `.exe`, with no installer** (the human's
  decision). There is no NSIS or MSI bundle.
- **The exe links C code.** `fm-tarpack` depends on `zstd` (bundled libzstd)
  and `liblzma` (or `xz2`; bundled liblzma), both compiled by the `cc` crate
  with MSVC. `flate2` is pure Rust.
- **Static C runtime.** By default MSVC binaries import `vcruntime140.dll`,
  which a clean machine may lack, and a portable exe cannot install the VC++
  redistributable. Link the CRT statically for
  `x86_64-pc-windows-msvc`, by adding to `.cargo/config.toml`:

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

**Rules that bind this task.**

- No secrets in the repository or the workflow. Signing keys, if they are ever
  added, come from CI secrets and are out of scope here.
- The end-to-end check uses throwaway files in a scratch folder, never real
  user data.
- **Extraction with `-P` writes to the real `/`.** In the check, it runs only
  on a disposable target: a throwaway container or VM, or a test device meant
  for deployment. It never runs on a developer's own Linux machine.

## Files

- `.cargo/config.toml`: static CRT for `x86_64-pc-windows-msvc` (above)
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
10. **On a disposable Linux target**, for each of the four files:
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
11. **Decompression cost on armv7.** On the armv7 target, or failing that an
    armv7 container under qemu (note which), run
    `/usr/bin/time -v tar --zstd -xpPf …` and the `.tar.xz` equivalent. Record
    the peak resident set size of each. Expect roughly 8 MiB (zstd) and 9 MiB
    (xz) above tar's own baseline. Record the wall time too.

## Acceptance criteria

- CI uploads the portable exe as an artifact on every push to the branch.
- The CI dependency check proves that the exe imports no C runtime, zstd, or
  liblzma DLL.
- `docs/tarpack-e2e.md` records all 11 steps passing, with the commit SHA
  tested.
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

## Risks

- `+crt-static` applies to every crate built for the target, including tests.
  Confirm that `cargo test` still passes on the Windows job after adding it.
- Artifact size: exclude debug symbols from the uploaded artifact. Consider
  `strip = true` and `lto = true` in the release profile if the size is a
  problem. Do not change opt-level without measuring.
- A missing WebView2 runtime makes the exe fail at startup. Only the README
  covers this; there is no in-app recovery.
