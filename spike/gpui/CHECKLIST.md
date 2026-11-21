# Spike A manual checklist (Windows 11)

Run with the `fm-spike.exe` artifact of the latest green `spike-gpui` run
(Actions > spike-gpui > run > Artifacts > `fm-spike`). Copy the exe alone to a
folder you can write to (for example `C:\spike`), and run it from there. Every
button's outcome shows in the window and is appended to
`fm-spike-results.txt` beside the exe: send that file back with this one.
Time needed: about 20 minutes.

Machine (fill in before starting):

- Windows edition / version / build (`winver`):
- Display scale (Settings > System > Display > Scale), and number of monitors:
- Default `.toml` handler installed (which app?):

Result lines: write `PASS`, `FAIL`, or a note.

## 0. Preparation

1. Create a scratch folder `C:\spike\files` with a few ordinary files and one
   sub-folder `sub`.
2. Create a file whose name contains a lone UTF-16 surrogate (PowerShell):

   ```powershell
   New-Item -Path ("C:\spike\files\lone-" + [char]0xD800 + "-surrogate.txt") -ItemType File
   ```

   Explorer shows it with a replacement glyph. Never delete real files.
3. Start `C:\spike\fm-spike.exe`. Expected: a window titled "FileManager",
   about 1000x700, not smaller than 800x560 when you shrink it. The first log
   line block lists the five fm-* crates (fm-core ... fm-dbmanager).

   Result: ______  (window title / size / min size ok?)

## A2. File dialogs

4. Click **Open file**, pick an ordinary file. Expected: the log shows the
   path as `{:?}` and `valid unicode: true`.
   Result: ______
5. Click **Open folder**, pick `C:\spike\files`. Expected: the folder path is
   logged, then one `entry` line per item (read with std). The lone-surrogate
   entry shows `valid unicode: false` and an escaped name like `\u{d800}`.
   This proves the std route keeps the name.
   Result: ______
6. Click **Open multiple files**, select two files with Ctrl-click. Expected:
   two paths.
   Result: ______
7. Click **Open file** again and pick the lone-surrogate file. Expected
   (per the source, this fails): an `ERROR` line, or a cancelled dialog. Write
   exactly what the log says.
   Result: ______
8. Click **Open file (rfd)** and pick the lone-surrogate file. Expected (per
   the source): a `PANICKED` line. Write what the log says.
   Result: ______
9. Click **Save (GPUI)**. Expected: a save dialog whose name box shows
   `gateway.tar.gz`. Note whether a file-type filter is shown (the source says
   only "All files"). Cancel or save to `C:\spike`.
   Result: ______
10. Click **Save with filter (rfd)**. Expected: name box `gateway.tar.gz`, type
    drop-down "Tar archive (gz)" (`*.gz`). Save to `C:\spike`; the log shows the
    chosen path.
    Result: ______

## A3. Drop from Explorer

11. Drag two ordinary files and the `sub` folder from Explorer over the
    window, without releasing. Expected: the accent-coloured "Drop files here"
    overlay appears.
    Result: ______
12. Move the cursor out of the window. Expected: the overlay disappears. Move
    back in: it reappears.
    Result: ______
13. Release over the window. Expected: overlay gone; log shows `Drop: 3
    path(s)` and one line per path, folder included.
    Result: ______
14. Drag the lone-surrogate file over the window and release. Expected (per
    the source): the overlay may or may not appear, and the drop logs
    `Drop: 0 path(s)` because GPUI skips non-UTF-16-valid names. Write what you
    see (also whether any error is shown).
    Result: ______
15. Drag the lone-surrogate file together with an ordinary file and release.
    Expected: only the ordinary file arrives.
    Result: ______

## A4. Window close

16. Close the window with the title-bar X while **Dirty** is off. Expected:
    closes at once and the process exits (check Task Manager). Restart the exe.
    Result: ______
17. Tick **Dirty**. Click the title-bar X. Expected: window stays; "Close
    without saving?" with **Keep editing** / **Close** appears.
    Result: ______
18. Press **Keep editing**. Expected: dialog gone, window open.
    Result: ______
19. Press Alt+F4. Expected: dialog appears again, window stays.
    Result: ______
20. Press **Close**. Expected: window closes and the process exits (Task
    Manager shows no `fm-spike.exe`).
    Result: ______

## A5. System editor and Explorer

21. Click **Open .toml in default app**. Expected: the default `.toml` handler
    opens `sample.toml` (in a temp folder).
    Result: ______ (which app?)
22. Click **Reveal .toml in Explorer**. Expected: Explorer opens with
    `sample.toml` selected.
    Result: ______
23. Click **Open folder**, pick `C:\spike\files` (this sets the "last picked"
    to the lone-surrogate entry), then **Reveal last picked/dropped**.
    Expected: Explorer opens with that file selected (not "file not found").
    Result: ______
24. Click **Open last picked/dropped**. Expected: the default handler for
    `.txt` opens it (or the "open with" prompt).
    Result: ______

## A7. Progress, responsiveness, watcher

25. Click **Simulated build (5 s)**. Immediately click into the text box and
    type a sentence quickly. Expected: the progress bar and counter advance
    smoothly (about every 50 ms), every character you type appears at once,
    and the log ends with `Simulated build finished: ... progress events`; the
    counter ends at `5000 / 5000 ms`.
    Result: ______ (events reported:   )
26. Click **Watch file**. Open the logged `sample.toml` path in an editor,
    change a character and save. Expected: "changed on disk" appears within
    about half a second of saving.
    Result: ______
27. Click **Edit watched file**. Expected: the log shows `Edited the watched
    file`, then `Watch: changed on disk`.
    Result: ______

## Wrap-up

28. Icon: the exe's icon in Explorer and the window's taskbar button show the
    FileManager icon (CI cannot check this; the build script embeds
    `icon.ico` as resource id 1, where GPUI looks for it).
    Result: ______
29. Rendering: note any flicker, blur, wrong DPI scaling, or slow redraw while
    resizing the window.
    Result: ______
30. Attach `fm-spike-results.txt` (beside the exe) with this file.

# Spike B manual checklist (Windows 11)

Same exe artifact family: the `fm-spike` artifact of the latest green
`spike-gpui` run, and the `fm-spike-shots` artifact (CI's PNGs and check
result). Fill in the machine block of part A first. Three entry points beyond
part A's check screen:

- `fm-spike.exe --pipeline`: the B6 screen in a window.
- `fm-spike.exe --widgets`: the B1-B5 widgets (10,000-row table included).
- `fm-spike.exe --shoot <dir> [--dark]`, `--verify <dir> <reference.png>`,
  `--diff <a.png> <b.png> <out.txt>`: B7. These write files, not a console
  (the exe is a GUI program): read `<dir>\*.shoot-result.txt`,
  `<dir>\check-result.txt` and the diff output file.

## B6. Themed screen, live theme switch

31. Start `fm-spike.exe --pipeline` with Windows in light mode (Settings >
    Personalization > Colors > Choose your mode). Expected: sidebar, folder
    row, buttons and four steps as in `docs/manual/images/pipeline-chosen.png`
    (text shapes may differ).
    Result: ______
32. Leave the window open and switch Windows to dark mode. Expected: the
    window recolours to the dark tokens within about a second, without a
    restart; no element stays light. Switch back to light: same.
    Result: ______ (delay, any element that did not change)
33. Resize the window down to 800 x 560 and up. Expected: sidebar keeps its
    12rem width, the layout stays usable, no clipped focus rings.
    Result: ______

## B7. Screenshot, on your machine

34. Note the display scale (Settings > System > Display) and run
    `fm-spike.exe --shoot C:\spike\shots` then again with `--dark`. Expected:
    `pipeline-chosen.png` and `pipeline-chosen-dark.png` appear, no window is
    left open. Open `pipeline-chosen.shoot-result.txt`: copy its `scale factor`
    and `image ... device px` lines here. Expected at 100%: 1000x700.
    Result: ______
35. Repeat step 34 at a display scale other than 100% (125% or 150%). Expected
    per the source: a larger PNG (device pixels), not 1000x700. Write the
    size you get.
    Result: ______
36. Run `--shoot C:\spike\shots2` and compare:
    `Get-FileHash C:\spike\shots\pipeline-chosen.png, C:\spike\shots2\pipeline-chosen.png`.
    Expected: identical hashes. If not, run `--diff` and write its output.
    Result: ______
37. The result file lists `font Segoe UI`, `font Cascadia Mono`, `font
    Consolas` as installed or not. Write the three lines. Open the PNG: is the
    folder path (monospace) in Cascadia Mono, Consolas or something else?
    Result: ______
38. Run `fm-spike.exe --verify C:\spike\shots C:\...\docs\manual\images\pipeline-chosen.png`
    (any checkout of the repo has the reference image). Read
    `C:\spike\shots\check-result.txt` and write the count on its last line and
    any FAIL lines.
    Result: ______

## B1-B5, B8. Widgets by hand (`--widgets`)

39. Text inputs: click the first field, type, select with the mouse and with
    Shift+Arrows, Ctrl+A, Ctrl+C, Ctrl+V a path from Explorer. The second field
    is read-only: its text can be selected but not changed. The third has the
    red (danger) border. Placeholder text of an empty field is muted. Enter
    shows `submitted: [...]` at the bottom.
    Result: ______
40. Checkbox: click it and press Space: unchecked, checked, "Not set" (dash and
    the words "Not set"), unchecked. Tab reaches it from the fields.
    Result: ______
41. Table: scroll with the wheel and by dragging the scrollbar through 10,000
    rows. Expected: no visible stalls; header stays. Click a row, then Up/Down.
    Hover a row. Write how it feels and any stall.
    Result: ______
42. Dialog: Tab to "Run all...", press Enter. Expected: scrim over the window,
    focus ring on Cancel, Escape closes and focus is back on "Run all...".
    Open again and click Run: the button shows busy and the dialog stays; Escape
    and Cancel do nothing (this demo never finishes the work: close the window
    to leave). Note the dialog's shadow.
    Result: ______
43. Menu: Tab to "Recent" and press Enter: two items with a muted second line.
    Down/Up move, Escape closes and "Recent" has focus again. Click "Shortcuts":
    a small popover under it, Escape closes it.
    Result: ______
44. Press Ctrl+Alt+Shift+D anywhere in the window: the bottom line flips
    `dev mode: false` / `true`.
    Result: ______
