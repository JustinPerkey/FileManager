# FileManager

A Windows desktop app (Tauri v2, Rust, React) that hosts several tools. The
first is the Tar Packager, which builds `.tar`, `.tar.gz`, `.tar.zst`, and
`.tar.xz` archives from a manifest, for extraction on a Linux target.

Developer commands and layout are in `CLAUDE.md`.

## Running the portable exe

FileManager is a single portable `.exe`. There is no installer: download
`FileManager-<version>-x64.exe` (the CI artifact, or `target/release/filemanager.exe`
from `npm run tauri:build`), put it anywhere, and run it. GitHub downloads the
CI artifact as `FileManager-<version>-x64.exe.zip`; unzip it to get
`FileManager-<version>-x64.exe`. The C runtime is linked statically, so no
VC++ redistributable and no `api-ms-win-crt-*` DLLs are needed.

- **WebView2.** The app needs the Evergreen WebView2 runtime, which ships with
  Windows 11 and current Windows 10. If the app fails to start on a machine that
  lacks it, install it from
  <https://developer.microsoft.com/microsoft-edge/webview2/>. The exe cannot
  install it itself.
- **SmartScreen.** The exe is not code-signed, so Windows SmartScreen shows a
  prompt on first run. Choose "More info", then "Run anyway".
- **Where state lives.** Not beside the exe. App state is in
  `%APPDATA%\FileManager\`, and manifests default to
  `%APPDATA%\FileManager\tarpack\manifests\`.

## Extracting an archive on the target

The archive stores absolute names (`/opt/...`), so `-P` is required. Extraction
writes to the real `/`, so run it as root on the intended target only. The app
shows the exact command after a build.

```sh
tar --no-overwrite-dir -xpPf archive.tar
tar -z --no-overwrite-dir -xpPf archive.tar.gz
tar --zstd --no-overwrite-dir -xpPf archive.tar.zst
tar -J --no-overwrite-dir -xpPf archive.tar.xz
```

The full command table, flags, and archive layout are in
`docs/tarpack-manifest.md`. The manual release check is `docs/tarpack-e2e.md`.
