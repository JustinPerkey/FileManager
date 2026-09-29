# M3 — Manifest model, parsing, and validation

Status: awaiting approval
Project: tarpack   Depends on: M1 (landed)

## Goal

Turn a manifest file into a validated `Manifest`, or into a list of errors and
warnings, each with a line and column. Define the `ArchiveFormat` enum shared by
the writer and the remembered state. Write the manifest and archive reference
doc that later tasks use as the single source of the format.

## Context

**The Tar Packager** builds a Linux `.tar` from Windows files. The **manifest**
is a TOML config file, edited by hand outside the program. It lists a fixed set
of files, and for each file the Linux directory it goes to inside the archive
and its permissions. The user later supplies the actual Windows file for each
entry (M5). This task only reads and validates the manifest.

### Format (version 1)

```toml
# Tar Packager manifest
version = 1
name = "Gateway deploy"
output_name = "gateway.tar.zst"  # optional: suggested Save name; a known archive suffix also picks the default format

[defaults]                       # optional table; each field optional; shown values are the built-in defaults
mode = "0644"                    # file permissions, octal string
dir_mode = "0755"                # permissions for directory entries the archive creates
uid = 0
gid = 0
uname = "root"
gname = "root"

[[file]]
id = "gateway-bin"               # stable key; last-used locations are stored against it
source = "gateway"               # expected Windows file name; used to match drops
dir = "/opt/gateway/bin"         # Linux directory inside the archive
# name = "gateway"               # optional rename; defaults to `source`
mode = "0755"                    # optional; defaults to defaults.mode
# uid, gid, uname, gname         # optional per-file overrides of defaults

[[file]]
id = "gateway-conf"
source = "gateway.conf"
dir = "/etc/gateway"
mode = "0640"
gname = "gateway"
gid = 990

[[file]]
id = "gateway-start"
source = "start.sh"
dir = "/opt/gateway/bin"
mode = "0755"
normalize_eol = true             # optional, per file only, default false: CRLF -> LF when writing
```

Required fields: top-level `version` and `name`; per file, `id`, `source`, and
`dir`. Zero `[[file]]` entries is valid, and produces a warning.

**Owners** (decided by the human): every entry carries a numeric uid and gid
plus user and group names. They default to `0`/`0` `root:root`.

**`normalize_eol`** (decided by the human): a per-file boolean, default
`false`. When `true`, the writer (M4) converts every CRLF pair in that file to
LF. When `false` or absent, the bytes are copied exactly. It is **not**
accepted in `[defaults]`: conversion must be a deliberate per-file choice.
`deny_unknown_fields` on the defaults table rejects it there.

**Archive paths** (decided by the human): the archive stores **absolute**
names, such as `/opt/gateway/bin/gateway`. The target extracts with GNU tar
using `-P`. This task does not write archives. It only needs the long-name
warning below to measure the stored name, leading `/` included, and the
reference doc to explain extraction.

### Validation

**Errors.** Each has a line and column, and names the entry `id` where there is
one. Any error makes the manifest unbuildable.

- `version` is not `1`.
- Unknown keys at any level (use `deny_unknown_fields`), so typos surface.
- A required field is missing, or a value has the wrong type.
- `id` is empty or duplicated.
- `dir`:
  - is not absolute (must start with `/`);
  - contains a `..` segment, a backslash, or NUL;
  - contains an empty segment other than the trailing one (`/opt//x` is an
    error).
- `name` or `source` is empty, contains `/` or `\` or NUL, or is `.` or `..`.
- `output_name` is present but empty, or contains `/`, `\`, or NUL.
- `normalize_eol` is not a boolean (the type error rule covers this).
- Two entries resolve to the same target path (`dir` + `/` + `name`, with
  `dir`'s trailing `/` trimmed). The comparison is case-sensitive, because the
  target is Linux.
- `mode` or `dir_mode` is not an octal string of 1 to 4 digits, or exceeds
  `07777`.
- `uid` or `gid` does not fit in `u32`.
- `uname` or `gname` is empty or longer than 32 bytes.

**Warnings.** These do not block a build.

- Two entries share a `source` name, compared case-insensitively. A drop cannot
  tell them apart, so the user must pick those files per row.
- A stored path is 100 bytes or longer. The stored path is the absolute target
  path with its leading `/`. The writer will emit a GNU long-name record. The
  message says "100 bytes or longer" and gives the byte count.
- The manifest has no `[[file]]` entries.

### Archive formats

The user builds one of four output formats. This task defines the enum and its
pure helpers in `crates/fm-tarpack/src/format.rs`. It writes no archives (M4
does) and stores no state (M5 does). Defining it here lets those two tasks run
in parallel against one type.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveFormat { Tar, TarGz, TarZst, TarXz }   // TS: "tar" | "tarGz" | "tarZst" | "tarXz"
```

- `ArchiveFormat::ALL: [ArchiveFormat; 4]`, in the order above.
- `extension(self) -> &'static str` returns `.tar`, `.tar.gz`, `.tar.zst`, or
  `.tar.xz`.
- `from_file_name(name: &OsStr) -> Option<ArchiveFormat>` recognises the known
  suffixes `.tar`, `.tar.gz`, `.tgz`, `.tar.zst`, and `.tar.xz`,
  ASCII-case-insensitively. `.tgz` maps to `TarGz`. Match the longest suffix
  first (`x.tar.gz` is `TarGz`, not `Tar`).
- `with_extension(self, name: &OsStr) -> OsString` strips a known archive suffix
  if there is one, then appends `self.extension()`. It works on `OsStr` bytes
  without lossy conversion.
- `extract_command(self, file_name: &str) -> String` returns the exact GNU tar
  command for the target. `file_name` is a display string, and is wrapped in
  single quotes when it contains anything outside `[A-Za-z0-9._+-]`:
  - Tar: `tar --no-overwrite-dir -xpPf <file>`
  - TarGz: `tar -z --no-overwrite-dir -xpPf <file>`
  - TarZst: `tar --zstd --no-overwrite-dir -xpPf <file>`
  - TarXz: `tar -J --no-overwrite-dir -xpPf <file>`
- **Compression parameters**, as public constants with their reasons in doc
  comments (M4 uses them; they are decided, do not change them):
  - `GZIP_LEVEL: u32 = 6`. Conventional default. gzip's window is fixed at
    32 KiB, so the target's decompression cost is negligible.
  - `ZSTD_LEVEL: i32 = 19` and `ZSTD_WINDOW_LOG: u32 = 23`. The target is a
    low-power armv7. zstd's decompression memory is set by the window, not the
    level. An 8 MiB window bounds it and stays within the decoder's default
    limit. Long mode and levels 20–22 are excluded, because they raise the
    window.
  - `XZ_PRESET: u32 = 6`. xz's default, with an 8 MiB dictionary and about
    9 MiB to decompress. Presets 7–9 need 17–65 MiB and are excluded.
- **Default format of a manifest:** `Manifest::default_format() -> ArchiveFormat`
  returns `from_file_name(output_name)` when `output_name` is present and has a
  known suffix, and otherwise `Tar`. (M5 layers "the last format the user chose
  for this manifest" on top of this.)

**Rules that bind this task.**

- The crate has no tauri dependency.
- Tests never touch real user files; fixtures are strings or files in a
  `TempDir`.
- Types that will cross to the UI derive `ts_rs::TS` and are exported to
  `apps/desktop/src/lib/generated/` by the existing export test from M1.
  Regenerate them, and never hand-edit.

## Files

- `crates/fm-tarpack/src/manifest/{mod.rs, raw.rs, model.rs, validate.rs, mode.rs}`
- `crates/fm-tarpack/src/format.rs`: `ArchiveFormat` and the helpers above
- `crates/fm-tarpack/src/lib.rs`
- `crates/fm-tarpack/Cargo.toml`: adds `toml` (with spans), `serde`, `sha2`,
  and `ts-rs`
- `examples/tarpack/example.toml`: the manifest above, valid
- `docs/tarpack-manifest.md`: **the reference for the format and for
  extraction.** Later task plans point here instead of restating the format. It
  covers:
  - every field and its default, including `normalize_eol` (per file only,
    CRLF pairs become LF, lone CR is kept, default is a byte-exact copy);
  - every error and warning, with an example message;
  - **Archive layout.** Names are stored absolute (`/opt/...`). Directory
    entries are emitted for every ancestor except `/`, with `dir_mode` and the
    default owner. The format is GNU, with long-name records for names of 100
    bytes or more. mtime is the source file's modification time.
  - **Output formats.** A table of the four formats, their extensions, and
    their compression settings, with the target decompression memory from the
    constants above. zstd is recommended for the armv7 target, because it
    decompresses fastest. How the default format is chosen.
  - **Extracting on the target.** The four commands from `extract_command`,
    and why each flag is there:
    - `-P` is **required**: without it, GNU and BusyBox tar strip the leading
      `/` and extract relative to the current directory.
    - `-p` applies the modes, and when run as root GNU tar also applies the
      owners.
    - `--no-overwrite-dir` keeps the mode and owner of directories that already
      exist on the target (such as `/etc`), while new directories still get
      `dir_mode`.
    - `--zstd` needs GNU tar 1.31 or newer and the `zstd` program.
    - A listing without extraction is `tar -tvPf <file>` (with the matching
      decompression flag).
- Regenerated `apps/desktop/src/lib/generated/*`

## Design

- `raw.rs`: serde structs mirroring the TOML, with `deny_unknown_fields` and
  `toml::Spanned` on the fields that need locations.
- `model.rs`:
  - `Manifest { name, output_name, entries: Vec<Entry> }` with
    `Entry { id, source, target_dir, target_name, mode: u32, uid, gid, uname, gname, normalize_eol: bool }`.
    Add `Entry::target_path() -> String`, the absolute stored name
    (`dir` with its trailing `/` trimmed, then `/`, then `name`).
    All fields are resolved, so defaults are already applied.
  - The fields are private, with getters.
  - Constructible only through validation, so no later code can hold an invalid
    manifest.
  - `dir_mode` and the default owner are also resolved and exposed.
- `Diagnostic { severity, line, col, entry_id: Option<String>, message }`
- `pub fn load(path: &Path) -> Result<LoadedManifest, LoadError>`:
  - `LoadedManifest { path, sha256: [u8; 32], manifest, warnings }`
  - `LoadError::Invalid(Vec<Diagnostic>)`, or an I/O error
  - The hash is of the exact file bytes. M6 compares it to detect edits made
    outside the app.
- `pub fn parse(text: &str) -> Result<(Manifest, Vec<Diagnostic>), Vec<Diagnostic>>`
  holds the pure core that the tests exercise.
- Export `Diagnostic`, `Severity`, `ArchiveFormat`, and a UI-facing
  `ManifestView` to TS. `ManifestView` holds the name plus entries as
  displayable strings: the mode rendered as `rwxr-xr-x` and in octal, and
  `normalize_eol` as a boolean.

## Acceptance criteria

- Every validation rule above has at least one passing and one failing case.
- Error messages name the entry `id` where there is one, and carry a line and
  column.
- All errors are reported together, not only the first.
- `examples/tarpack/example.toml` loads with no errors and no warnings.
- `normalize_eol` is accepted per file, defaults to `false`, and is rejected in
  `[defaults]` with a location.
- `ArchiveFormat` suffix recognition, extension rewriting, and extraction
  commands match the lists above exactly.
- `docs/tarpack-manifest.md` covers every field, every diagnostic, all four
  formats, and the extraction commands with `-P`.

## Tests proving completion

`cargo test -p fm-tarpack manifest`, table-driven, including:

- `rejects_relative_dir`
- `rejects_dotdot`
- `rejects_backslash_in_dir`
- `rejects_duplicate_id`
- `rejects_duplicate_target`
- `rejects_unknown_key_with_location`
- `rejects_mode_out_of_range`
- `mode_parses_octal_strings`
- `defaults_apply_and_per_file_overrides_win`
- `warns_on_shared_source_name`
- `warns_on_long_target`
- `collects_all_errors`
- `example_manifest_is_valid`
- `load_hashes_exact_bytes`
- `normalize_eol_defaults_false_and_parses_per_file`
- `normalize_eol_rejected_in_defaults`
- `rejects_output_name_with_separator`
- `long_name_warning_counts_leading_slash`: a stored path of exactly 100
  bytes, `/` included, warns; 99 bytes does not

`cargo test -p fm-tarpack format`:

- `format_from_file_name_longest_suffix_wins` (`a.tar.gz`, `A.TGZ`,
  `a.tar.zst`, `a.tar.xz`, `a.tar`, `a.zip` gives `None`)
- `with_extension_replaces_known_suffix_or_appends`
- `extract_command_per_format_includes_absolute_names_flag`
- `extract_command_quotes_unsafe_names`
- `default_format_follows_output_name_suffix`

Also run `generated_types_are_current` and
`cargo clippy -p fm-tarpack --all-targets -- -D warnings`.

## Out of scope

- Matching Windows files (M5).
- Writing archives (M4).
- File watching and Tauri commands (M6).

## Risks

- The format constants are decisions, not tuning knobs. Changing them changes
  the target's decompression memory.
- `version = 1` is frozen once this lands. A future format change adds
  `version = 2` handling, and never reinterprets version 1 files.
