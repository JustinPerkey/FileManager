# Tar Packager end-to-end check

Manual checklist for the portable Windows exe (task M7). Run it on Windows
(steps 1-10) and a disposable Linux target (steps 11-12). Use throwaway files in
a scratch folder, never real user data.

**Extraction with `-P` writes to the real `/`.** Run steps 11 and 12 only on a
disposable target: a throwaway container or VM, or a test device meant for
deployment. Never on a developer's own Linux machine.

- Commit SHA tested: _(fill in)_
- CI artifact name: _(fill in, `FileManager-<version>-x64.exe`)_
- Tester / date: _(fill in)_
- Windows version (step 10): _(fill in)_
- Monospace font (step 10): _(Cascadia Mono / Consolas / could not tell)_
- Step 12 ran on: _(armv7 device / armv7 container under qemu)_

## Steps

1. Copy the CI artifact exe to a clean Windows machine (no Rust, no Visual
   Studio, no VC++ redistributable) and run it. It starts, with only the
   SmartScreen prompt.
2. Open `examples/tarpack/example.toml`, copied to a scratch folder. It shows no
   errors, and the entry with `normalize_eol` is marked.
3. Drop a folder containing some of the listed files. The matched, unmatched,
   and ambiguous results are correct.
   After the drop, the file list stays visible, with the matched files marked
   ready.
4. Assign one remaining file with Browse. Give the `normalize_eol` entry a file
   saved with CRLF line endings.
5. The default format follows the example's `output_name` (`.tar.zst`). Choose
   an output path and create the archive. The summary shows the entry count, the
   on-disk and uncompressed sizes, and the SHA-256; the extraction command with
   `-P` (shown only in developer mode: press Ctrl+Alt+Shift+D first, which also
   shows Edit in editor and its Ctrl+E shortcut); and the normalised entry with its CRLF count. Press both Copy buttons
   and paste into Notepad: the hash and the command arrive exactly. The UI uses
   `navigator.clipboard.writeText` with no clipboard plugin. If Copy fails in
   the packaged exe, record the failure and report it (a separate backend task).
6. Build again to the same path. The overwrite confirmation appears, and Cancel
   leaves the old file untouched.
7. Switch the format to each of `.tar`, `.tar.gz`, and `.tar.xz`, and build
   each. The Save name and output path extension follow the format every time.
8. Close and reopen the app. The manifest, all sources, and the last chosen
   format are restored. A deleted source shows as Missing.
9. Edit the manifest in an external editor (change a mode). The banner appears.
   Building before reload is refused. After reload, the new mode is shown.
10. **WebView2 middle truncation**, in the packaged exe. Assign sources from a
    scratch folder nested deep enough that the Windows location column
    truncates, and choose an output path in a similarly deep folder so the build
    bar's output path truncates too. Record the Windows version and, if you can
    tell, the monospace font (Cascadia Mono or Consolas). For every truncated
    path, in the table and in the build bar:
    - "…" sits directly against the file name's leading `\`, with no
      gap;
    - the file name is never split mid-word.

    Check at several window widths, resizing a pixel or two at a time over at
    least 20 px each time, in **both table layouts** (stacked at the default
    window, columns when maximised), and again at **200% text** (Windows
    Settings > Accessibility > Text size, then restart the app). If the app's
    text does not grow, record "200% text: not applied by WebView2" instead of
    passing or failing, and report it; do not change zoom settings. Do not focus
    a table row or hover a path while checking (both show the full path by
    design). Restore the text size afterwards.

    On failure, record the window width, text size, layout, and a screenshot
    path (kept outside the repository). Do not change `MiddlePath` or any UI
    file in this task; report a follow-up UI task.
11. **On a disposable Linux target**, for each of the four files:
    - Run `tar -tvPf` plus the format flag. Every name begins with `/`; modes
      and owners match the manifest.
    - Run the exact extraction command shown by the app, as root. Check that
      files land at their absolute paths with the manifest's modes and owners;
      new parent directories have `dir_mode`; a pre-existing directory (for
      example `/etc`) keeps its original mode and owner; the normalised script
      has no CR bytes (`grep -c $'\r'` gives `0`) and runs.
12. **Decompression cost on armv7.** On the armv7 target, or an armv7 container
    under qemu (note which), run `/usr/bin/time -v tar --zstd -xpPf archive.tar.zst` and
    `/usr/bin/time -v tar -J -xpPf archive.tar.xz`. Record peak resident set size and wall time. Expect
    roughly 8 MiB (zstd) and 9 MiB (xz) above tar's own baseline.

## Results

| Step | Result (pass/fail) | Notes |
| ---- | ------------------ | ----- |
| 1    |                    |       |
| 2    |                    |       |
| 3    |                    |       |
| 4    |                    |       |
| 5    |                    |       |
| 6    |                    |       |
| 7    |                    |       |
| 8    |                    |       |
| 9    |                    |       |
| 10   |                    |       |
| 11 (.tar)     |           |       |
| 11 (.tar.gz)  |           |       |
| 11 (.tar.zst) |           |       |
| 11 (.tar.xz)  |           |       |
| 12 (zstd RSS / time) |    |       |
| 12 (xz RSS / time)   |    |       |

Step 10 detail (one row per layout and text size):

| Layout | Text size | Window widths tried | Result | Screenshot path |
| ------ | --------- | ------------------- | ------ | --------------- |
|        |           |                     |        |                 |
