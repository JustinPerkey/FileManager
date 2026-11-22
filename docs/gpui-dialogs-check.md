# GPUI file dialogs: manual check (Windows 11)

The native open and save dialogs (`apps/filemanager/src/platform/win_dialog.rs`)
cannot be exercised in CI. This check runs them for real. Run it on Windows 11
after any change to `win_dialog.rs`, and record the result in the PR.

## Setup

In PowerShell, make a folder and files whose names hold a lone surrogate:

```powershell
New-Item -Path C:\check -ItemType Directory -Force
New-Item -Path ("C:\check\lone-" + [char]0xD800 + ".toml") -ItemType File
New-Item -Path C:\check\plain.toml -ItemType File
New-Item -Path C:\check\other.txt -ItemType File
New-Item -Path ("C:\check\folder-" + [char]0xD800) -ItemType Directory
```

## Run

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
   - Type `report` (no extension) and save: the result ends in `report.gz`.
     Type a name of an existing file: Windows asks whether to replace it.

## Pass

`dialog-check.txt` (in the folder you ran from) holds three lines. The open and
folder lines contain `\u{d800}` inside the path (the name was not mangled and
the dialog did not fail), and the save line ends in `report.gz`. A cancelled
dialog reads `Ok(None)`.

Any other outcome is a FAIL and reopens milestone M3c.
