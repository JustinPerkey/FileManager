# M3 — Manifest model, parsing, and validation

Status: awaiting approval
Project: tarpack   Depends on: M1 (landed)

## Goal

Turn a manifest file into a validated `Manifest`, or into a list of errors and
warnings, each with a line and column. Write the manifest reference doc that
later tasks use as the single source of the format.

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
output_name = "gateway.tar"      # optional: suggested file name in the Save dialog

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
```

Required fields: top-level `version` and `name`; per file, `id`, `source`, and
`dir`. Zero `[[file]]` entries is valid, and produces a warning.

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
- A target path is longer than 100 bytes. The writer will use GNU long-name
  headers.
- The manifest has no `[[file]]` entries.

**Rules that bind this task.**

- The crate has no tauri dependency.
- Tests never touch real user files; fixtures are strings or files in a
  `TempDir`.
- Types that will cross to the UI derive `ts_rs::TS` and are exported to
  `apps/desktop/src/lib/generated/` by the existing export test from M1.
  Regenerate them, and never hand-edit.

## Files

- `crates/fm-tarpack/src/manifest/{mod.rs, raw.rs, model.rs, validate.rs, mode.rs}`
- `crates/fm-tarpack/src/lib.rs`
- `crates/fm-tarpack/Cargo.toml`: adds `toml` (with spans), `serde`, `sha2`,
  and `ts-rs`
- `examples/tarpack/example.toml`: the manifest above, valid
- `docs/tarpack-manifest.md`: **the reference for the format.** Every field,
  its default, every error and warning with an example message, and a note that
  archive paths are written relative (the leading `/` is stripped; extract with
  `tar -xpf <file> -C /`). Later task plans point here instead of restating the
  format.
- Regenerated `apps/desktop/src/lib/generated/*`

## Design

- `raw.rs`: serde structs mirroring the TOML, with `deny_unknown_fields` and
  `toml::Spanned` on the fields that need locations.
- `model.rs`:
  - `Manifest { name, output_name, entries: Vec<Entry> }` with
    `Entry { id, source, target_dir, target_name, mode: u32, uid, gid, uname, gname }`.
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
- Export `Diagnostic`, `Severity`, and a UI-facing `ManifestView` (the name
  plus entries as displayable strings, with the mode rendered as `rwxr-xr-x`
  and octal) to TS.

## Acceptance criteria

- Every validation rule above has at least one passing and one failing case.
- Error messages name the entry `id` where there is one, and carry a line and
  column.
- All errors are reported together, not only the first.
- `examples/tarpack/example.toml` loads with no errors and no warnings.
- `docs/tarpack-manifest.md` covers every field and every diagnostic.

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

Also run `generated_types_are_current` and
`cargo clippy -p fm-tarpack --all-targets -- -D warnings`.

## Out of scope

- Matching Windows files (M5).
- Writing archives (M4).
- File watching and Tauri commands (M6).

## Risks

- `version = 1` is frozen once this lands. A future format change adds
  `version = 2` handling, and never reinterprets version 1 files.
