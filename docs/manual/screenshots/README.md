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
