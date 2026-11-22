# Manual screenshots

`../images/*.png` are taken from the real frontend running in Chromium, with
the Tauri backend replaced by `mock.js` (a stand-in for `window.__TAURI_INTERNALS__`
that returns canned sessions for each scenario). No Rust code runs, so the
shots work on any OS.

To regenerate them after a UI change:

```sh
npm install                                   # once, at the repo root
npm run dev -w apps/desktop -- --port 1420 --strictPort &
node docs/manual/screenshots/shoot.js         # all shots
node docs/manual/screenshots/shoot.js tarpack # only names starting with "tarpack"
```

`shoot.js` needs Playwright with Chromium (`require('playwright')`); it is not
a project dependency. The Tar Packager shots use a 1366×800 window (wide
enough for the table layout), the others the app's default 1000×700. The
fonts are swapped for Inter and DejaVu Sans Mono so Linux renders close to
Segoe UI and Cascadia Mono.

When a shot's scenario needs data the mock does not have, add it to the
scenario switches in `mock.js` rather than editing the frontend.

## GPUI harness

The GPUI app has its own screenshot harness, `filemanager-shoot`, which renders
the real app (fixture backends, a scripted fake platform, no real files or
dialogs) to PNG without Node or a browser. It replaces the scripts above at the
manual cutover milestone; until then both exist.

```sh
cargo run -p filemanager-gpui --features shoot --bin filemanager-shoot -- --out shots
# options: --only name[,name...]   --prefix <p>   --dark (no effect until the theme task)
```

It writes `<out>/<prefix><scenario>.png` (the scenario's name is the PNG's
stem) and prints `ok <name>` or `FAIL <name>: <reason>` per scenario; it exits
non-zero if any scenario failed. Scenarios live in
`apps/filemanager/src/shoot/scenarios/`; the API is documented in
`apps/filemanager/src/shoot/mod.rs`.

- It needs Windows: GPUI renders to an image only on the Windows (and macOS)
  renderers, and the harness refuses to run at a display scale other than 100%
  (the PNG would have the wrong size).
- It is a dev-profile build on purpose, and the `shoot` feature (GPUI's
  `test-support`) is never in the release exe; CI checks that
  `target\release\filemanager-shoot.exe` does not exist.
- CI (the windows job) runs it twice into `shots/run1` and `shots/run2`,
  compares the PNGs byte for byte into `shots/determinism.txt` (a difference is
  reported, not failed), and uploads `shots/` as the `screenshots` artifact.
