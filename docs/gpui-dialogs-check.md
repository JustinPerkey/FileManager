# GPUI file dialogs: manual check (Windows 11)

The native open and save dialogs (`apps/filemanager/src/platform/win_dialog.rs`)
cannot be exercised in CI. This check runs them for real. Run it on Windows 11
after any change to `win_dialog.rs` or `platform/gate.rs`, and record the
result in the PR. A FAIL reopens milestone M3c.

## Prerequisites

On the Windows 11 machine: Rust (stable, via rustup) and the MSVC toolchain
(Visual Studio Build Tools, "Desktop development with C++"), and a checkout of
this repository. `cargo run` below builds everything else.

## Setup

In PowerShell, make a folder and files whose names hold a lone surrogate:

```powershell
New-Item -Path C:\check -ItemType Directory -Force
New-Item -Path ("C:\check\lone-" + [char]0xD800 + ".toml") -ItemType File
New-Item -Path C:\check\plain.toml -ItemType File
New-Item -Path C:\check\other.txt -ItemType File
New-Item -Path C:\check\existing.gz -ItemType File
New-Item -Path ("C:\check\folder-" + [char]0xD800) -ItemType Directory
Remove-Item C:\check\dialog-check.txt -ErrorAction SilentlyContinue
```

## Run 1: pick, filters, extension, modality

```powershell
cargo run -p filemanager-gpui --example dialog_check -- C:\check
```

A blank window opens, then three dialogs appear one after another.

1. Open dialog, titled "Open a manifest".
   - It starts in `C:\check`.
   - The file-type list shows only "Manifest (*.toml)": `plain.toml` and
     `lone-<surrogate>.toml` are listed, `other.txt` is not.
   - While it is open, the blank window cannot be clicked or focused (the
     dialog is modal to it).
   - Pick the `lone-` file.
2. Folder picker, starting in `C:\check`. Pick the `folder-` directory.
3. Save dialog.
   - It starts in `C:\check` with the name `gateway.tar.gz` filled in.
   - The file-type list shows "Gzip archive (.tar.gz)".
   - Type `existing` (no extension) and Save: Windows asks whether to replace
     `existing.gz`. Answer No, then type `report` and Save.

Pass: `C:\check\dialog-check.txt` holds three lines. The `open` and `folder`
lines contain `\u{d800}` inside the path (the name was not mangled and the
dialog did not fail), and the `save` line ends in `report.gz`.

## Run 2: cancel

```powershell
Remove-Item C:\check\dialog-check.txt
cargo run -p filemanager-gpui --example dialog_check -- C:\check
```

Press Esc in each of the three dialogs.

Pass: `dialog-check.txt` holds `open: Ok(None)`, `folder: Ok(None)` and
`save: Ok(None)`.

## Run 3: one dialog at a time

```powershell
Remove-Item C:\check\dialog-check.txt
cargo run -p filemanager-gpui --example dialog_check -- C:\check --click
```

The window shows a Browse button. Double-click it quickly.

Pass: exactly one dialog opens, and the window stays disabled until it closes.
Cancel the dialog and close the window. `dialog-check.txt` holds
`click 2: Ok(None)` (the second click was refused at once) and then
`click 1: Ok(None)`.
