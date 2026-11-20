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

28. Rendering: note any flicker, blur, wrong DPI scaling, or slow redraw while
    resizing the window.
    Result: ______
29. Attach `fm-spike-results.txt` (beside the exe) with this file.
