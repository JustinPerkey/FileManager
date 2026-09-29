# Tar Packager manifest and archive reference

The single source for the manifest format, the archive layout, the output
formats, and extraction on the target. A working example is
`examples/tarpack/example.toml`.

A **manifest** is a TOML file, edited by hand outside the program. It lists a
fixed set of files and, for each, the Linux directory it goes to inside the
archive and its permissions. The user supplies the actual Windows file for each
entry when packaging.

## Format (version 1)

```toml
version = 1
name = "Gateway deploy"
output_name = "gateway.tar.zst"

[defaults]
mode = "0644"
dir_mode = "0755"
uid = 0
gid = 0
uname = "root"
gname = "root"

[[file]]
id = "gateway-bin"
source = "gateway"
dir = "/opt/gateway/bin"
mode = "0755"
```

`version = 1` is frozen. A future change adds `version = 2` handling and never
reinterprets version 1 files. Unknown keys are errors at every level, so typos
surface.

### Top level

| Field | Required | Meaning |
|---|---|---|
| `version` | yes | Must be `1`. |
| `name` | yes | Display name. Must not be empty. |
| `output_name` | no | Suggested Save name. Must not be empty or contain `/`, `\` or NUL. A known archive suffix also picks the default format. |
| `[defaults]` | no | Table. Every field optional. |
| `[[file]]` | no | Zero or more entries. Zero is valid and warns. |

### `[defaults]`

| Field | Default | Meaning |
|---|---|---|
| `mode` | `"0644"` | File permissions, octal string of 1 to 4 digits. |
| `dir_mode` | `"0755"` | Permissions of directory entries the archive creates. |
| `uid` | `0` | Numeric owner. Must fit in `u32`. |
| `gid` | `0` | Numeric group. Must fit in `u32`. |
| `uname` | `"root"` | Owner name, 1 to 32 bytes. |
| `gname` | `"root"` | Group name, 1 to 32 bytes. |

`normalize_eol` is **not** accepted here: conversion must be a deliberate
per-file choice. It is reported as an error at its key, with a message saying
it is set per file.

### `[[file]]`

| Field | Required | Default | Meaning |
|---|---|---|---|
| `id` | yes | | Stable key; last-used locations are stored against it. Not empty, unique. |
| `source` | yes | | Expected Windows file name, used to match drops. Not empty, no `/`, `\` or NUL, not `.` or `..`. |
| `dir` | yes | | Linux directory inside the archive. Absolute; no `.`, `..`, or empty segments; no backslash or NUL. |
| `name` | no | `source` | Rename inside the archive. Same rules as `source`. |
| `mode` | no | `defaults.mode` | Permissions. |
| `uid`, `gid`, `uname`, `gname` | no | the defaults | Per-file owner overrides. |
| `normalize_eol` | no | `false` | Per file only. When `true`, every CRLF pair becomes LF when writing; a lone CR is kept. When `false` or absent, the bytes are copied exactly. |

The stored path of an entry is `dir` with its trailing `/` trimmed, then `/`,
then `name`, for example `/opt/gateway/bin/gateway`. A file's stored path may
not also be a directory of another entry (for example `/opt/gateway` as a file
while another entry has `dir = "/opt/gateway/bin"`).

## Diagnostics

Every diagnostic has a line and column (1-based). Parsing happens in two stages:

1. A **TOML syntax error** stops at the first one: nothing past it can be read
   reliably.
2. Everything else is reported together, sorted by line and column: unknown
   keys at every level, missing required fields, wrong types, and every
   validation rule below, across all entries. The one limit: inside a single
   `[defaults]` or `[[file]]` table only the **first** wrong-type or
   missing-field error is reported, and that table's value rules (`dir`,
   `mode`, owner, and so on) are not checked until it is fixed. Unknown keys in
   that table are still reported, and so are all errors in other tables.

Every error inside a `[[file]]` table names its `id`: the message starts with
`` file `<id>`: ``. A table without a **non-empty** string `id` starts with
`[[file]] #<n>: ` (`n` is its 1-based position), for every error from that
table, including `id must not be empty`.

### Errors

| Rule | Example message |
|---|---|
| `version` is not `1` | `unsupported version 2; this program reads version 1` |
| Unknown key | `` unknown key `colour` `` / `` file `a`: unknown key `colour` `` / `` [defaults]: `normalize_eol` is set per file, not in [defaults] `` |
| Missing required field | `` file `a`: missing field `dir` `` / `` missing field `version` `` |
| Wrong type | `` file `a`: invalid type: integer `644`, expected a string `` |
| `id` empty | `[[file]] #2: id must not be empty` |
| `id` duplicated; reported on every table that shares the id, each at its own `id` | `` file `a`: duplicate id (first used on line 4) `` on each later table, and `` file `a`: duplicate id (also used on lines 8, 12) `` on the first |
| `dir` not absolute | `` file `a`: dir must be absolute (start with `/`) `` |
| `dir` has a backslash or NUL | `` file `a`: dir must not contain a backslash or NUL `` |
| `dir` has a `..` segment | `` file `a`: dir must not contain a `..` segment `` |
| `dir` has a `.` segment | `` file `a`: dir must not contain a `.` segment `` |
| `dir` has an empty segment (other than the trailing one) | `` file `a`: dir must not contain an empty segment (`//`) `` |
| `name` or `source` empty, has `/`, `\`, NUL, or is `.`/`..` | `` file `a`: source must not be `.` or `..` `` |
| `output_name` empty or has `/`, `\`, NUL | `` output_name must not be empty or contain `/`, `\` or NUL `` |
| Two entries with the same stored path (case-sensitive); reported on both entries, each at its own `name` (else `dir`) | `` file `b`: target path `/x/a` is already used by file `a` `` on the later, `` file `a`: target path `/x/a` is also used by file `b` `` on the earlier. With three or more entries on one path, each pair is reported once on each of its two entries |
| An entry's stored path is also a directory of another entry; reported on both entries: on the file entry at its `name` (else `dir`), and on the directory entry at its `dir` | `` file `a`: target path `/opt/gateway` is also a directory of file `b` `` and `` file `b`: dir needs `/opt/gateway`, which is the target path of file `a` `` |
| Bad `mode` or `dir_mode` | `` file `a`: mode: mode `0898` is not an octal string `` / `` defaults.dir_mode: mode `12345` must be an octal string of 1 to 4 digits `` |
| `uid` or `gid` does not fit in `u32` | `` file `a`: uid `-3` does not fit in u32 `` |
| `uname` or `gname` empty or over 32 bytes | `` file `a`: gname must be 1 to 32 bytes, got 33 `` |
| Manifest `name` empty | `name must not be empty` |

### Warnings

They do not block a build.

| Rule | Example message |
|---|---|
| Two entries share a `source`, compared case-insensitively. A drop cannot tell them apart, so the user picks those files per row. | `` file `b`: source `cfg.TXT` matches file `a` (compared case-insensitively); a dropped file cannot tell them apart, so pick their files per row `` |
| A stored path is 100 bytes or longer, leading `/` included. The writer emits a GNU long-name record. | `` file `a`: stored path `/...` is 100 bytes or longer (153 bytes); the archive will use a GNU long-name record `` |
| No `[[file]]` entries | `the manifest has no [[file]] entries` |

### When the manifest has errors

A manifest with errors is still shown, as far as it can be trusted.

- **Passed entries.** A `[[file]]` table passes when no error is attributed to
  it. Warnings never fail an entry. Passed entries keep manifest order.
- **Failed entries.** Each `[[file]]` table with at least one error becomes one
  failure record: its position, its `id` and `source` when they are strings,
  the line of its `[[file]]` header, and every error of the table.
- **Cross-entry errors fail every entry involved.** Every table sharing a
  duplicate `id` fails; both entries of a duplicate target fail; both the file
  entry and the directory entry of a file/directory collision fail.
- **Only tables that resolved are compared.** A table that failed on its own
  takes no part in the duplicate-target, file/directory, shared-source, and
  long-name checks, so fixing it can reveal a new collision. The duplicate-id
  check uses the raw `id`, so it covers failed tables too, and a table that
  fails only through a duplicate id is still compared.
- **Errors that hide every entry**, because no entry can be read as its author
  meant it: a TOML syntax error or a file that is not UTF-8; `version` missing,
  of the wrong type, or not `1`; any error in `[defaults]` (every entry
  inherits it), including `defaults` that is not a table and `normalize_eol`
  placed there; an unknown top-level key (it may be a misspelt `[defaults]`,
  such as `[default]`); and `file` that is not an array. Entry failures found
  in the same pass are still listed; after a syntax error there are none.
- **Errors that do not hide entries**, because they do not change how an entry
  is read: `name` missing, of the wrong type, or empty (the name is then empty),
  and a bad `output_name` (it is then ignored).

**Building while errors exist.** Errors do not block a build. The archive holds
only the entries that passed. Every failed entry and every manifest-level error
is listed as an error in the build's result, together with the warnings, so a
file the manifest lists is never left out without the result naming it. Nothing
about the errors is written into the archive or beside it. When no entry passed
(entries hidden, every entry failed, or none listed) there is nothing to
build. Warnings never block.

## Archive layout

- Names are stored **absolute** (`/opt/gateway/bin/gateway`).
- Directory entries are emitted for every ancestor of an entry except `/`, with
  `dir_mode` and the default owner.
- The format is GNU tar, with long-name records for names of 100 bytes or more.
- A file's mtime is its source file's modification time, in whole seconds.
  Every directory entry gets the newest mtime among the archived files.
- An entry with `normalize_eol = true` has every CRLF rewritten to LF as it is
  written; the header's size is the converted size.

## Output formats

| Format | Extension | Compression | Target decompression memory |
|---|---|---|---|
| Tar | `.tar` | none | none |
| TarGz | `.tar.gz` | gzip level 6 | negligible (fixed 32 KiB window) |
| TarZst | `.tar.zst` | zstd level 19, window log 23 (8 MiB) | bounded by the 8 MiB window |
| TarXz | `.tar.xz` | xz preset 6 (8 MiB dictionary) | about 9 MiB |

The constants live in `crates/fm-tarpack/src/format.rs` and are decisions, not
tuning knobs. The target is a low-power armv7. zstd's decompression memory is
set by the window, not the level, so long mode and levels 20-22 are excluded
because they raise the window. xz presets 7-9 need 17-65 MiB and are excluded.
**zstd is recommended for the armv7 target**, because it decompresses fastest.

**Default format.** `output_name`'s suffix picks it when it is one of `.tar`,
`.tar.gz`, `.tgz`, `.tar.zst`, `.tar.xz` (ASCII-case-insensitive, longest suffix
first); otherwise the default is Tar. The last format the user chose for a
manifest overrides this.

## Extracting on the target

```sh
tar --no-overwrite-dir -xpPf archive.tar
tar -z --no-overwrite-dir -xpPf archive.tar.gz
tar --zstd --no-overwrite-dir -xpPf archive.tar.zst
tar -J --no-overwrite-dir -xpPf archive.tar.xz
```

- `-P` is **required**. Without it, GNU and BusyBox tar strip the leading `/`
  and extract relative to the current directory.
- `-p` applies the modes, and when run as root GNU tar also applies the owners.
- `--no-overwrite-dir` keeps the mode and owner of directories that already
  exist on the target (such as `/etc`), while new directories still get
  `dir_mode`.
- `--zstd` needs GNU tar 1.31 or newer and the `zstd` program.
- A listing without extraction is `tar -tvPf <file>`, with the matching
  decompression flag (`-z`, `--zstd`, `-J`).

A file name outside `[A-Za-z0-9._+-]` is wrapped in single quotes in the
commands the program shows.
