# M3 — Manifest model, parsing, and validation

Status: implemented in f04c008; **review follow-up pending** (see "Review
follow-up" at the end)
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
The unknown-key walk rejects it there, with a message saying it is set per
file (`deny_unknown_fields` on `RawDefaults` remains as a backstop).

**Archive paths** (decided by the human): the archive stores **absolute**
names, such as `/opt/gateway/bin/gateway`. The target extracts with GNU tar
using `-P`. This task does not write archives. It only needs the long-name
warning below to measure the stored name, leading `/` included, and the
reference doc to explain extraction.

### Validation

**Errors.** Each has a line and column. Every error that arises inside a
`[[file]]` table, **including unknown keys, wrong types, and missing required
fields**, names that entry's `id`: `Diagnostic.entry_id` is set and the message
starts with ``file `<id>`: ``. (If the table has no string `id`, `entry_id` is
`None` and the message starts with `[[file]] #<n>: `, `n` being the 1-based
position of the table.) Any error makes the manifest unbuildable.

- `version` is not `1`.
- Top-level `name` is empty. (Only non-empty is required. The separator and
  `.`/`..` rules below apply to the per-file `name`, not to this display name.)
- Unknown keys at any level, so typos surface. `normalize_eol` inside
  `[defaults]` is an unknown key there.
- A required field is missing, or a value has the wrong type.
- `id` is empty or duplicated.
- `dir`:
  - is not absolute (must start with `/`);
  - contains a `..` segment, a `.` segment, a backslash, or NUL (`/opt/./x`,
    `/opt/.`, and `/.` are errors; `/opt/.x` and `/opt/x.` are fine). A `.`
    segment is rejected because `/opt/./x` and `/opt/x` name the same
    directory, which would defeat the duplicate-target check;
  - contains an empty segment other than the trailing one (`/opt//x` is an
    error).
- Per-file `name` or `source` is empty, contains `/` or `\` or NUL, or is `.`
  or `..`.
- `output_name` is present but empty, or contains `/`, `\`, or NUL.
- `normalize_eol` is not a boolean (the type error rule covers this).
- Two entries resolve to the same target path (`dir` + `/` + `name`, with
  `dir`'s trailing `/` trimmed). The comparison is case-sensitive, because the
  target is Linux. The error is reported on the later entry, located at its
  `name` value when `name` is set, otherwise at its `dir` value.
- An entry's target path is also a directory of another entry: it equals
  another entry's `dir` (trailing `/` trimmed) or any ancestor of it. For
  example, A has `dir = "/opt"`, `name = "gateway"` and B has
  `dir = "/opt/gateway/bin"`. The archive would need `/opt/gateway` to be both
  a file and a directory. This is an error whatever the order of the two
  entries. It is reported on the entry whose **file** path collides (A), names
  the other entry, and is located like the duplicate-target error. Comparison
  is case-sensitive and by whole segments (`/opt/gateway` does not conflict
  with `/opt/gatewayx/bin`).
- `mode` or `dir_mode` is not an octal string of 1 to 4 digits. (Four octal
  digits cannot exceed `07777`, so there is no separate range rule.)
- `uid` or `gid` does not fit in `u32`.
- `uname` or `gname` is empty or longer than 32 bytes.

**Which errors are reported together.** Parsing happens in two stages.

1. The text is parsed as TOML into a spanned document. A **TOML syntax error**
   stops here, and only the first one is reported: nothing past it can be
   read reliably.
2. Everything else is collected in one pass and reported together, sorted by
   line and column: unknown keys at every level, missing required fields,
   wrong types, and every validation rule above, across all entries. The one
   limit: inside a single `[defaults]` table or a single `[[file]]` table,
   only the **first** wrong-type or missing-field error of that table is
   reported (serde stops at the first), and that table's value rules
   (`dir`, `mode`, owner, and so on) are not checked until it deserializes.
   Unknown keys in the same table are still reported, and so are all errors in
   other tables. A broken `[[file]]` table's `id`, when it is a string, still
   takes part in the duplicate-id check.

`docs/tarpack-manifest.md` states exactly this.

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
- `filter_extension(self) -> &'static str` returns `"tar"`, `"gz"`, `"zst"`,
  or `"xz"`: the last suffix without its dot. This is the form a native
  Save-dialog filter takes, and Windows matches only the last suffix. M6 sends
  it to the UI, so the UI never derives it.
- `from_file_name(name: &OsStr) -> Option<ArchiveFormat>` recognises the known
  suffixes `.tar`, `.tar.gz`, `.tgz`, `.tar.zst`, and `.tar.xz`,
  ASCII-case-insensitively. `.tgz` maps to `TarGz`. Match the longest suffix
  first (`x.tar.gz` is `TarGz`, not `Tar`).
- `with_extension(self, name: &OsStr) -> OsString` strips a known archive suffix
  if there is one, then appends `self.extension()`. It works on the `OsStr`
  without lossy conversion and **without `unsafe`**: on Unix through
  `std::os::unix::ffi::{OsStrExt, OsStringExt}` (bytes), on Windows through
  `std::os::windows::ffi::{OsStrExt, OsStringExt}` (`encode_wide` /
  `from_wide`). Every known suffix is ASCII, so its length is the same in
  bytes and in UTF-16 units, and cutting it off leaves a valid `OsString`.
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
- **No `bigint` crosses the boundary.** ts-rs generates `u64`/`i64` as TS
  `bigint`. Any 64-bit integer field in an exported type (for example if
  `Diagnostic.line`/`col` are `u64`) carries `#[ts(type = "number")]`. Prefer
  `u32` for line, column, and mode.
- Types that will cross to the UI derive `ts_rs::TS` and are exported to
  `apps/desktop/src/lib/generated/` by the export test M1 created:
  - The test lives in the shell crate, at
    `apps/desktop/src-tauri/src/generated_types.rs`.
  - Append one entry per root type to its `EXPORTERS` list, written
    `<fm_tarpack::path::Type as ts_rs::TS>::export_all`. `export_all` also
    writes the types a root references.
  - Regenerate with the command recorded in `CLAUDE.md` Commands:
    `UPDATE_GENERATED=1 cargo test -p filemanager --lib generated_types_are_current`.
  - Commit the result. Never hand-edit a generated file.
  - Do not use `#[ts(export)]`.
  - Building the shell crate on Linux needs the webkit2gtk dev packages that
    `CLAUDE.md` lists.
- `ts-rs` is a workspace dependency. Add it to the crate as
  `ts-rs = { workspace = true }`, and never pin a version in the crate. Derive
  it as `ts_rs::TS` directly; `fm-core` does not re-export it.

## Files

- `crates/fm-tarpack/src/manifest/{mod.rs, raw.rs, model.rs, validate.rs, mode.rs}`
- `crates/fm-tarpack/src/format.rs`: `ArchiveFormat` and the helpers above
- `crates/fm-tarpack/src/lib.rs`
- `CLAUDE.md`: Layout lines for `crates/fm-tarpack/src/{manifest/, format.rs}`,
  `examples/tarpack/`, and `docs/tarpack-manifest.md` (nothing else)
- `crates/fm-tarpack/Cargo.toml`: adds `toml` (with spans), `serde`, `sha2`,
  and `ts-rs = { workspace = true }`
- `apps/desktop/src-tauri/src/generated_types.rs`: append the exporters for
  `Diagnostic`, `Severity`, `ArchiveFormat`, and `ManifestView` to `EXPORTERS`.
  Change nothing else in the file.
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
- Regenerated `apps/desktop/src/lib/generated/*`, written by the regenerate
  command above

## Design

- `raw.rs`: serde structs for `[defaults]` (`RawDefaults`) and one `[[file]]`
  table (`RawFile`), with `deny_unknown_fields` kept as a backstop and
  `toml::Spanned` on the fields that need locations. The document is first
  parsed with `toml::de::DeTable::parse(text)` (a `Spanned<DeTable>` whose keys
  and values carry byte spans). A walk over that table reports unknown keys,
  removes them, and then deserializes each piece on its own:
  - top-level `version` and `name` into `Spanned<i64>` / `Spanned<String>`,
    `output_name` into `Spanned<String>`, each with
    `T::deserialize(value.into_deserializer())`;
  - `[defaults]` into `RawDefaults`;
  - each element of the `file` array into `RawFile`.

  A `toml::de::Error` from one of these carries a span in the original text
  (a wrong type points at the value; a missing field points at the table,
  which for `[[file]]` is its `[[file]]` header), so each becomes a located
  diagnostic, and a `[[file]]` one is attributed to that table's `id`.
  `file` that is not an array of tables, or `defaults` that is not a table, is
  a located type error.
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
  `ManifestView` to TS. This is the shape the UI contract (M6, and the UI
  tasks) consumes, so it is fixed exactly:

  ```rust
  pub struct ManifestView {            // TS: { name, outputName, entries }
      pub name: String,
      pub output_name: Option<String>, // TS: outputName: string | null
      pub entries: Vec<EntryView>,
  }
  pub struct EntryView {
      pub id: String,
      pub source: String,
      pub target_path: String,         // absolute stored name, e.g. "/opt/gateway/bin/gateway"
      pub mode: String,                // four octal digits, e.g. "0755"
      pub mode_text: String,           // symbolic, e.g. "rwxr-xr-x" (s/S, t/T for special bits)
      pub owner: String,               // "<uname>:<gname>", e.g. "root:root"
      pub uid: u32,                    // numeric ids that are written to the header
      pub gid: u32,
      pub normalize_eol: bool,
  }
  ```

  Both derive `Serialize`, `ts_rs::TS` with `#[serde(rename_all = "camelCase")]`,
  so TS sees `targetPath`, `modeText`, `normalizeEol`, `outputName`.
  `ManifestView` has **no** `defaultFormat`: the format the UI shows is the
  session's `format` (the remembered choice, falling back to
  `Manifest::default_format()`), which M6 supplies. `Manifest::default_format()`
  itself stays in Rust. M6 builds its session entries by flattening `EntryView`
  (`#[serde(flatten)]`) and adding its own `assigned` and `status`, so
  `EntryView` is the single definition of those fields.

## Acceptance criteria

- Every validation rule above has at least one passing and one failing case.
- Error messages name the entry `id` where there is one, and carry a line and
  column. This includes unknown keys, wrong types, and missing fields inside a
  `[[file]]` table.
- All errors are reported together, not only the first, within the limits
  stated under "Which errors are reported together".
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
- `rejects_dot_segment_in_dir`
- `rejects_backslash_in_dir`
- `rejects_duplicate_id`
- `rejects_duplicate_target` (also asserts the error's location is the `name`
  value when `name` is set, and the `dir` value otherwise)
- `rejects_target_that_is_another_entrys_directory`
- `rejects_unknown_key_with_location`
- `file_table_errors_name_the_entry_id`
- `unknown_keys_are_reported_with_validation_errors`
- `rejects_empty_manifest_name`
- `rejects_mode_out_of_range`
- `mode_parses_octal_strings`
- `defaults_apply_and_per_file_overrides_win`
- `warns_on_shared_source_name`
- `warns_on_long_target`
- `collects_all_errors`
- `example_manifest_is_valid` (reads the example with `include_str!` and runs
  `parse`, not `load`: no file is read at run time)
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
- `with_extension_and_from_file_name_keep_non_unicode_names`
  (`#[cfg(unix)]` and `#[cfg(windows)]` variants)
- `filter_extension_is_last_suffix_without_dot`
- `extract_command_per_format_includes_absolute_names_flag`
- `extract_command_quotes_unsafe_names`
- `default_format_follows_output_name_suffix`

Also run:

- `cargo test -p filemanager generated_types_are_current` (check mode, after
  regenerating), and confirm that `lib/generated/` holds the four types and
  what they reference;
- `cargo clippy --workspace --all-targets -- -D warnings`, because the shell
  crate changed too.

Both CI jobs run `cargo test --workspace`, so they run the currency check
too.

## Out of scope

- Matching Windows files (M5).
- Writing archives (M4).
- File watching and Tauri commands (M6).

## Risks

- The format constants are decisions, not tuning knobs. Changing them changes
  the target's decompression memory.
- `version = 1` is frozen once this lands. A future format change adds
  `version = 2` handling, and never reinterprets version 1 files.

## Review follow-up

M3 is already implemented and committed (f04c008). The review found the gaps
below. The sections above are now the corrected spec; this list is the
concrete work to bring the **existing** code into line with them. Do not
rewrite what already works: every item names the file and what changes. Finish
with the full check list at the end, then hand the diff to the reviewer.

### F1. Errors inside a `[[file]]` table name the entry id (reviewer B1)

Today `parse` in `crates/fm-tarpack/src/manifest/mod.rs` runs
`toml::from_str::<RawManifest>` and turns any failure into one diagnostic via
`validate::syntax_error`, which sets `entry_id: None`. Change it to the
two-stage parse described under Design (`raw.rs` bullet):

- Parse with `toml::de::DeTable::parse(text)`. On `Err`, return
  `Err(vec![syntax_error(text, &e)])` as today; this path is TOML syntax only.
- Walk the spanned table (F3) and deserialize each piece on its own with
  `serde::de::IntoDeserializer`: `RawFile::deserialize(elem.clone().into_deserializer())`
  per `file` element, `RawDefaults` likewise, and the top-level scalars as
  `Spanned<i64>` / `Spanned<String>`. (Verified against toml 1.1.6: a wrong
  type in a `[[file]]` gives a span on the value, e.g. `mode = 644` gives
  "invalid type: integer `644`, expected a string" at the `644`; a missing
  `dir` gives "missing field `dir`" at the `[[file]]` header span. The
  document-level span is `0..0`, so a missing top-level field locates at 1:1.)
- Before deserializing a `[[file]]` element, read its `id` from the raw table
  if it is a string. Attribute every diagnostic from that table to it, through
  the existing `Ctx::push` prefix (``file `<id>`: ``). Without a string `id`,
  use the prefix `[[file]] #<n>: ` (1-based) and `entry_id: None`.
- Replace `syntax_error`'s role for serde errors with a helper that takes the
  `toml::de::Error`, the id, and pushes into `Ctx`. `RawManifest` is no longer
  deserialized as a whole; remove it, or keep it only if something still
  needs it. Keep `deny_unknown_fields` on `RawDefaults` and `RawFile`.
- `validate` takes the pieces (top-level scalars as `Option<Spanned<_>>`,
  `Option<RawDefaults>`, and per file its index, raw id and span, and
  `Option<RawFile>`) instead of a `RawManifest`. The duplicate-id check uses
  the raw id, so it still covers tables that failed to deserialize. The
  target, source, and long-name checks run only on resolved entries.
- Update the `parse` doc comment to state the new rule (syntax stops at the
  first; everything else collected; one type/missing-field error per table).

Tests (in `crates/fm-tarpack/src/manifest/tests.rs`):

- New `file_table_errors_name_the_entry_id`: for each of
  `colour = "red"` (unknown key), `mode = 644` (wrong type), and a table
  without `dir` (missing field) inside `[[file]]` with `id = "a"`, assert
  `entry_id == Some("a")`, the message starts with ``file `a`: ``, and the
  line is the key's line (unknown key, wrong type) or the `[[file]]` header
  line (missing field). A table with `id = 3` gives `entry_id == None` and a
  message starting `[[file]] #1: `.
- Update `rejects_type_and_missing_field_errors` to keep its single-error
  cases and assert the entry id where the error is inside `[[file]]`.

### F2. `.` segments in `dir` are errors (reviewer B2)

In `bad_dir` in `validate.rs`, reject a segment equal to `.` with the message
``must not contain a `.` segment``, checked in the same loop as `..`.
New test `rejects_dot_segment_in_dir`: `/opt/./x`, `/opt/.`, `/.`, `/./opt`
fail; `/opt/.x`, `/opt/x.`, `/opt/..x` pass.

### F3. Unknown keys are reported alongside everything else (reviewer B3)

Today one unknown key (serde `deny_unknown_fields`) aborts the whole parse
and hides every other error. In the walk from F1, before deserializing:

- Top level: allowed keys `version`, `name`, `output_name`, `defaults`,
  `file`. Each other key is an error at the **key's** span, message
  ``unknown key `<key>` ``, and is removed before deserializing.
- `[defaults]` (when it is a table): allowed `mode`, `dir_mode`, `uid`, `gid`,
  `uname`, `gname`. Message ``[defaults]: unknown key `<key>` ``. For
  `normalize_eol` specifically, use
  ``[defaults]: `normalize_eol` is set per file, not in [defaults]``.
- Each `[[file]]` table: allowed `id`, `source`, `dir`, `name`, `mode`, `uid`,
  `gid`, `uname`, `gname`, `normalize_eol`. Message
  ``unknown key `<key>` `` with the entry prefix from F1.
- `file` present but not an array, or an element that is not a table, and
  `defaults` present but not a table: a located type error at the value; the
  offending piece is skipped.

`DeTable` keys are `Spanned<DeString>`; collect the unknown keys first, then
`remove` them (the map's `retain` does not accept a closure over the spanned
key type).

Tests:

- New `unknown_keys_are_reported_with_validation_errors`: one manifest with
  a top-level unknown key, a `[defaults]` unknown key, a `[[file]]` unknown
  key, a relative `dir` in another entry, and `version = 2`. All five errors
  come back, sorted by line.
- Keep `rejects_unknown_key_with_location` and
  `normalize_eol_rejected_in_defaults` passing (location stays at the key,
  e.g. `(4, 1)`); update their message expectations if they match on serde's
  wording.
- Extend `collects_all_errors` with an unknown key and a wrong-type field in
  a second `[[file]]`, and assert both appear together with the existing
  errors.

### F4. Top-level `name` rule is only "non-empty"

The code already does this. Add `rejects_empty_manifest_name` (empty fails;
`name = "a/b"` and `name = "."` are accepted, since separator rules are
per-file only) so the rule has its pass and fail case.

### F5. Target that is another entry's directory (reviewer N5)

In `validate`, after resolving entries, build the set of directory paths each
resolved entry needs: its `dir` with the trailing `/` trimmed and every
ancestor of that except `/`. For each resolved entry whose `target_path()` is
in that set, push an error on that entry naming the other one, e.g.
``file `a`: target path `/opt/gateway` is also a directory of file `b` ``,
located like F6. Report each colliding pair once, independent of manifest
order. New test `rejects_target_that_is_another_entrys_directory`: A
(`dir = "/opt"`, `name = "gateway"`) with B (`dir = "/opt/gateway/bin"`) fails
in both orders with `entry_id == Some("a")`; B with `dir = "/opt/gateway"`
exactly fails too; `/opt/gateway` against `/opt/gatewayx/bin` passes.

### F6. Duplicate-target location (reviewer N6)

The duplicate-target error currently points at `file.dir.span()`. Point it at
the `name` value's span when `name` is set, otherwise at `dir`. Extend
`rejects_duplicate_target` to assert the column/line for both cases.

### F7. `EntryView` / `ManifestView` shape (reviewer N1)

In `crates/fm-tarpack/src/manifest/model.rs`, change the views to the exact
shape under Design: `ManifestView { name, output_name, entries }` (remove
`default_format`) and
`EntryView { id, source, target_path, mode, mode_text, owner, uid, gid, normalize_eol }`,
with `mode` as `format!("{:04o}", mode)`, `mode_text` as `mode::symbolic(mode)`,
and `owner` as `format!("{uname}:{gname}")`. Remove `mode_symbolic`,
`mode_octal`, `uname`, and `gname` from the view (they stay available on
`Entry`). Keep `Manifest::default_format()` and its test. Update
`view_renders_mode_and_target` to assert `mode == "0755"`,
`modeText`/`mode_text == "rwxr-xr-x"`, `owner == "root:root"`, and, with
`gname = "gateway"`, `gid = 990` set, `owner == "root:gateway"` and
`gid == 990`. Then regenerate `apps/desktop/src/lib/generated/` with the
regenerate command and commit the result (`EntryView.ts`, `ManifestView.ts`
change). `EXPORTERS` in `apps/desktop/src-tauri/src/generated_types.rs` does
not change.

### F8. No `unsafe` in `with_extension`, and a non-UTF-8 test (reviewer N3)

`crates/fm-tarpack/src/format.rs` (around lines 95-97) uses
`unsafe { OsString::from_encoded_bytes_unchecked(..) }`. Replace it with the
cfg'd safe implementation described under Archive formats
(`#[cfg(unix)]` bytes via `OsStrExt::as_bytes` / `OsStringExt::from_vec`;
`#[cfg(windows)]` UTF-16 via `encode_wide` / `OsString::from_wide`, dropping
the suffix's length in units). `known_suffix` may keep reading
`as_encoded_bytes()`, which is safe. Add
`with_extension_and_from_file_name_keep_non_unicode_names`:

- `#[cfg(unix)]`: `OsStr::from_bytes(b"\xffname.TGZ")` gives
  `from_file_name == Some(TarGz)`, and `TarZst.with_extension` of it equals
  `OsStr::from_bytes(b"\xffname.tar.zst")`.
- `#[cfg(windows)]`: `OsString::from_wide(&[0xD800, 'n' as u16, '.' as u16, 't' as u16, 'a' as u16, 'r' as u16])`
  (a lone surrogate) gives `Some(Tar)`, and `TarXz.with_extension` of it,
  run through `encode_wide`, equals the same prefix followed by `.tar.xz`.

Both CI jobs run `cargo test --workspace`, so each variant runs on its
platform.

### F9. Example test reads no file at run time (reviewer N4)

`example_manifest_is_valid` builds a path from `CARGO_MANIFEST_DIR` and calls
`load`. Change it to
`parse(include_str!("../../../../examples/tarpack/example.toml"))` (the path
is relative to `crates/fm-tarpack/src/manifest/tests.rs`), assert no errors
and no warnings, and keep the existing field assertions.
`load_hashes_exact_bytes` keeps covering `load` in a `TempDir`.

### F10. `mode.rs` (reviewer N8)

In `crates/fm-tarpack/src/manifest/mode.rs`, after the length check (1 to 4)
and the all-octal-digits check, the value cannot exceed `0o7777`. Remove the
unreachable "exceeds 07777" branch: compute the value by folding the digits
(or `from_str_radix` with an `expect` explaining why it cannot fail). Update
the doc comment to "an octal string of 1 to 4 digits". Keep
`mode_parses_octal_strings` and `rejects_mode_out_of_range` passing.

### F11. `docs/tarpack-manifest.md`

- **Diagnostics intro (lines ~79-82).** Replace with the two-stage rule
  exactly as under "Which errors are reported together": a TOML syntax error
  stops at the first; everything else is reported together; inside one
  `[defaults]` or `[[file]]` table only the first wrong-type or missing-field
  error is reported and that table's value rules wait until it is fixed.
  State that every error inside a `[[file]]` table names its `id`.
- **Error table (lines ~86-104).** Today the example messages nest single
  backticks inside single-backtick spans, which breaks the rendering. Every
  example message that contains a backtick is written as two backticks, a
  space, the message, a space, two backticks, so it renders as one span:

      | `id` duplicated | `` file `a`: duplicate id (first used on line 4) `` |

  Update the rows to the new messages:
  - Unknown key: ``unknown key `colour` `` /
    ``file `a`: unknown key `colour` `` /
    ``[defaults]: `normalize_eol` is set per file, not in [defaults]``.
  - Missing required field: ``file `a`: missing field `dir` ``.
  - Wrong type: ``file `a`: invalid type: integer `644`, expected a string``.
  - `dir` has a `..` or `.` segment: add the `.` row/wording.
  - Target path is another entry's directory: new row with the F5 message.
  - Bad `mode`/`dir_mode`: no "exceeds" wording.
- **`[defaults]` table (line ~52).** `mode` is "octal string of 1 to 4
  digits"; drop "at most `07777`".
- **`[[file]]` table.** `dir` row: "Absolute; no `.`, `..`, or empty
  segments; no backslash or NUL." Add a sentence that a file's stored path may
  not also be a directory of another entry.
- **Top level.** `name`: "Display name. Must not be empty." (unchanged, now
  explicitly the only rule).

### F12. `CLAUDE.md` Layout (reviewer N7)

In the Layout block of `CLAUDE.md`, add:

- under `crates/fm-tarpack/`: `src/manifest/` (parse, validate, model, views)
  and `src/format.rs` (`ArchiveFormat`, compression constants);
- a top-level `examples/tarpack/` line: example manifests, valid, used by
  tests through `include_str!`;
- a `docs/tarpack-manifest.md` line: the manifest, archive-layout, and
  extraction reference.

Change nothing else in `CLAUDE.md`.

### Checks before handing to the reviewer

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p fm-tarpack
UPDATE_GENERATED=1 cargo test -p filemanager --lib generated_types_are_current
cargo test --workspace            # includes the currency check in check mode
npm run typecheck && npm run lint && npm run test
```

The frontend checks matter because the generated `EntryView.ts` and
`ManifestView.ts` change. Nothing under `apps/desktop/src/` outside
`lib/generated/` consumes them yet; if something does, report it instead of
editing UI code. Confirm with `grep -rn unsafe crates/fm-tarpack` that no
`unsafe` remains.
