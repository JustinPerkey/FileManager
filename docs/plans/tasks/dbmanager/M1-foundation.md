# M1 — `fm-dbmanager` crate, bundled SQLite, and database opening

Status: implemented; the reviewer requested changes. **Run "Review round 1"
(end of this plan)** on this task plan; the rules below are already corrected
for it. (Approved 2026-11-16; revised the same day: the recent list holds
databases only; `fm-core` gains the app-wide config directory.)
Project: dbmanager   Depends on: none

## Goal

Add the `fm-dbmanager` crate with a statically linked SQLite, a `Database`
type that opens a user's SQLite file safely, and the tool's remembered state,
and wire the crate into CI and `CLAUDE.md`.

## Context

FileManager is getting a fourth tool, the **Database Manager** (`dbmanager`):
the user picks a SQLite file; one app-wide TOML config at a fixed location
(`%ProgramData%\FileManager\dbmanager.toml`, not user-selectable) says which
tables and fields are editable; the app shows tables as lists of objects and
objects as forms. This milestone lays only the crate and its lowest layer.
Later milestones add schema introspection, the config, validation, reads,
writes and the Tauri module.

Follow `CLAUDE.md` "Adding a tool", step 1, and its Invariants. In particular:

- The crate has **no tauri dependency** and depends on no other tool crate
  (`fm-tarpack`, `fm-schedule`, `fm-pipeline`). It may depend on `fm-core`.
- `[lints] workspace = true`; `cargo fmt` clean; clippy pedantic with
  `-D warnings`. Silence only with a scoped
  `#[expect(clippy::<lint>, reason = "...")]`.
- Tests that touch the filesystem run in a temp dir they create
  (`tempfile`, a workspace dependency).
- Paths are `PathBuf`/`&Path`, never display strings, except where a string
  is shown to the user.

### Bundled, static SQLite

- Use `rusqlite` with features `bundled` and `backup` (the backup API is used
  by a later milestone). `bundled` compiles SQLite's C amalgamation through
  the `cc` crate on every platform, so tests never depend on a system SQLite.
- Pick the newest `rusqlite` whose declared `rust-version` is not newer than
  the workspace's `rust-version = "1.90"` (root `Cargo.toml`). Commit the
  updated `Cargo.lock` (CI runs with `--locked`).
- `.cargo/config.toml` already sets `+crt-static` for
  `x86_64-pc-windows-msvc`, so `cc` builds SQLite with `/MT` exactly like
  zstd and liblzma. Do not set `RUSTFLAGS` anywhere and do not touch
  `tauri.conf.json`'s `staticVCRuntime`.
- The Windows CI job runs `dumpbin /dependents` on the exe and fails on a
  forbidden DLL. Add `sqlite3\S*\.dll` to its regex and to its step name/error
  text. (The shell will only link this crate's code from a later milestone;
  the check is in place before then.)

### `Database` (`src/db.rs`)

```rust
pub struct Database { /* rusqlite::Connection, path, read_only reason */ }

// Variants are tuple-like and carry the path (as implemented; accepted in review).
pub enum OpenError {
    NotAbsolute(PathBuf),          // relative paths resolve against an invisible cwd
    NotAFile(PathBuf),             // missing, or a directory
    NotUnicode(PathBuf),           // SQLite on Windows needs a UTF-8 path
    NotADatabase(PathBuf, String), // SQLite says "file is not a database", or similar
    Busy(PathBuf),                 // still locked by another program after busy_timeout
    Io(PathBuf, String),           // anything else, with SQLite's message
}

impl Database {
    pub fn open(path: &Path) -> Result<Database, OpenError>;
    pub fn path(&self) -> &Path;
    /// Some(reason) when the file could only be opened read-only.
    pub fn read_only_reason(&self) -> Option<&str>;
    pub fn conn(&self) -> &rusqlite::Connection;
    pub fn conn_mut(&mut self) -> &mut rusqlite::Connection;
    /// PRAGMA schema_version.
    pub fn schema_version(&self) -> rusqlite::Result<i64>;
}
```

Rules for `open`:

1. `path` must be absolute and an existing regular file; check before calling
   SQLite.
2. Open with `SQLITE_OPEN_READ_WRITE` and **without** `SQLITE_OPEN_CREATE`
   and without `SQLITE_OPEN_URI` (a file name must never be interpreted as a
   URI). Never create a file.
3. **Immediately after the connection opens, before any statement**, set
   `busy_timeout` 3000 ms (`conn.busy_timeout`). It is a per-connection
   setting, not a persistent setting of the file. It must come before the
   probe in rule 5: otherwise a database locked by another program (for
   example a rollback-journal database with a pending write) fails at once
   with `SQLITE_BUSY`, where the contract is "busy after 3 s".
4. **Read-only detection:**
   - If read-write fails with `SQLITE_READONLY` or `SQLITE_CANTOPEN` while the
     file exists, retry with `SQLITE_OPEN_READ_ONLY` (then rule 3 again).
     Also, after opening, treat `conn.is_readonly(DatabaseName::Main)` as
     read-only. Either way the reason is "the file is read-only".
   - **Folder probe:** a writable file in a read-only folder opens read-write,
     but cannot be written: SQLite must create its journal beside the file,
     and the app writes its backup there. So after a read-write open, probe
     the parent folder by creating a temp file in it with
     `tempfile::Builder::new().prefix(".fm-dbmanager-probe-").tempfile_in(parent)`
     and dropping it at once. That creates nothing persistent: the file is
     deleted on drop. If the probe fails with `PermissionDenied` (or
     `ReadOnlyFilesystem`), reopen with `SQLITE_OPEN_READ_ONLY` and set the
     reason "the folder is read-only, so changes cannot be saved". Any other
     probe error is ignored: the database opens read-write, and a later write
     reports its own error.
5. Prove it is a database: `SELECT count(*) FROM sqlite_schema`.
   - `SQLITE_BUSY`/`SQLITE_LOCKED`, still failing after the timeout →
     `OpenError::Busy(path)`. A later milestone maps it to the `Busy` kind.
   - `SQLITE_NOTADB` or similar → `NotADatabase`.
   - Anything else → `Io`.
6. Then apply, in this order: `PRAGMA foreign_keys = ON` (off by default per
   connection), `PRAGMA trusted_schema = OFF`, and
   `SQLITE_DBCONFIG_DEFENSIVE` on
   (`conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)`).
   **Never change `journal_mode`** or any persistent setting of the file.
7. A path that is not valid Unicode is `NotUnicode` (check with
   `path.to_str()` before opening, on every platform).

`OpenError` implements `Display` (a sentence naming the path via
`path.display()`) and `std::error::Error`. It is not a boundary type; a later
milestone maps it to the UI error.

### `RememberedState` (`src/state.rs`)

Persisted through `fm_core::Store` (see `crates/fm-core/src/store.rs`) at
`<app data>/dbmanager/state.json` (tool `"dbmanager"`, name `"state"`).

```rust
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RememberedState {
    /// Recent databases, most recent first, unique by path, at most MAX_RECENT.
    /// (The config is not remembered: there is one fixed app-wide config file.)
    pub recent: Vec<PathBuf>,
}
pub const MAX_RECENT: usize = 10;
impl StateData for RememberedState { const SCHEMA_VERSION: u32 = 1; }
impl RememberedState {
    /// Moves `database` to the front, removing any equal entry; truncates to
    /// MAX_RECENT. Paths that are not valid Unicode are ignored (the list must
    /// stay serialisable).
    pub fn touch(&mut self, database: &Path);
    pub fn forget(&mut self, database: &Path);
}
```

"Same database path" compares paths case-insensitively on Windows
(`#[cfg(windows)]`, ASCII case fold of the `to_str()` form) and exactly
elsewhere. Do not canonicalise (a file on a missing share must still be
recognised).

### `fm-core`: the app-wide config directory

The Database Manager reads one config for the whole app from a machine-wide
location (a later milestone uses it; it is shared, tool-agnostic code, so it
belongs in `fm-core`). Add to `crates/fm-core/src/dirs.rs`:

```rust
/// The machine-wide FileManager configuration directory. The app only reads
/// from it (developer tooling may create files there).
/// - Windows: `<ProgramData>\FileManager`, where `<ProgramData>` is the
///   `ProgramData` environment variable (normally `C:\ProgramData`); it must be
///   an absolute path, else `FmError::NoProgramDataDir`.
/// - Other platforms (development builds only): `/etc/filemanager`.
pub fn system_config_dir() -> Result<PathBuf>;
```

Add `FmError::NoProgramDataDir` (with its `kind()` string). Do not read any
other environment variable, and do not create the directory. Tests do not
call it against the real location: test the pure part by factoring
`fn system_config_dir_from(program_data: Option<OsString>) -> Result<PathBuf>`
(Windows logic, compiled on every platform) and testing that.

### CI and documentation

- `.github/workflows/ci.yml`, linux job, step "fm-* crates must not depend on
  tauri": add `fm-dbmanager` to the `for c in ...` list.
- `.github/workflows/ci.yml`, windows job, dumpbin step: forbid
  `sqlite3\S*\.dll` too.
- `CLAUDE.md`:
  - Status: mention the Database Manager (`dbmanager`) as in progress
    ("crate only; UI to come" style, matching how the other tools are
    described).
  - Layout: add a `crates/fm-dbmanager/` entry ("Database Manager domain
    library. Depends on fm-core only. No tauri dependency. `db.rs` opens a
    SQLite file; `state.rs` is the recent list, persisted as dbmanager/state").
  - Commands → native prerequisites: `fm-dbmanager` depends on `rusqlite`
    with `bundled`, which compiles SQLite's C source with the same toolchain
    and static CRT as zstd/liblzma; the dumpbin check forbids `sqlite3*.dll`.
  - The CI paragraph that lists forbidden DLLs: add `sqlite3*`.
  - The Linux note that cargo can be scoped to `-p fm-core -p fm-tarpack`:
    add `-p fm-dbmanager`.

## Files

Create:

- `crates/fm-dbmanager/Cargo.toml` (workspace package fields as in
  `crates/fm-schedule/Cargo.toml`; deps `fm-core` by path, `rusqlite`
  (`bundled`, `backup`), `serde` with `derive`; dev-dep `tempfile`
  (workspace); `[lints] workspace = true`)
- `crates/fm-dbmanager/src/lib.rs` (module docs: "Database Manager domain
  library. Must never depend on tauri.")
- `crates/fm-dbmanager/src/db.rs`, `crates/fm-dbmanager/src/state.rs`, and
  their tests (inline `#[cfg(test)] mod tests` or `src/tests/…`)

Edit: `crates/fm-core/src/dirs.rs`, `crates/fm-core/src/error.rs`,
`crates/fm-core/src/lib.rs` (re-export), `Cargo.lock`,
`.github/workflows/ci.yml`, `CLAUDE.md`.

Do not touch `apps/desktop/` (no shell dependency yet), any other tool crate,
or anything under `docs/plans/`.

## Acceptance criteria

- `cargo build -p fm-dbmanager` compiles SQLite from source (no system
  library) on Linux.
- `Database::open` never creates a file: opening a missing path returns
  `NotAFile` and the path still does not exist afterwards.
- A text file renamed `.db` fails with `NotADatabase`.
- After `open`, `PRAGMA foreign_keys` returns 1 and `PRAGMA trusted_schema`
  returns 0; `PRAGMA journal_mode` is unchanged from before the open (test a
  database created in WAL mode stays WAL, and one in DELETE stays DELETE).
- A database file marked read-only (`std::fs::Permissions::set_readonly`)
  opens with `read_only_reason()` set; a normal one opens with `None`.
- A relative path is `NotAbsolute`.
- `RememberedState::touch` keeps most-recent-first order, de-duplicates by
  database path, caps at 10; `forget` removes; a `Store` round trip
  preserves it.
- `system_config_dir_from(Some(p))` for an absolute `p` on the host platform
  (use a temp dir's path; never the real ProgramData) gives `p/FileManager`;
  `None`, an empty value or a relative value gives `NoProgramDataDir`. On
  non-Windows, `system_config_dir()` is `/etc/filemanager` (assert the path
  only; never create it).
- `cargo tree -p fm-dbmanager -i tauri -e normal,build,dev` reports that
  tauri matches no packages.
- CI's two edits are present; `CLAUDE.md` describes the crate.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, and `cargo test --workspace` pass.

## Tests proving completion

In `fm-dbmanager`, each in its own temp dir:

- `open_rejects_relative_path`, `open_missing_file_is_not_created`,
  `open_directory_is_not_a_file`, `open_non_database_fails`
- `open_sets_pragmas` (foreign_keys = 1, trusted_schema = 0)
- `open_preserves_journal_mode_wal`, `open_preserves_journal_mode_delete`
- `open_readonly_file_falls_back_to_read_only` (skip with a printed note when
  running as root on Unix, where permissions do not bind)
- `schema_version_changes_after_ddl`
- `recent_touch_orders_dedupes_and_caps`, `recent_forget_removes`,
  `recent_round_trips_through_store`
- in `fm-core`: `system_config_dir_from_program_data`,
  `system_config_dir_rejects_missing_or_relative`

Run: `cargo test -p fm-dbmanager`, then the full workspace commands in
`CLAUDE.md`.

## Out of scope

- Schema introspection (M2), the TOML config (M3), values and validation
  (M4), and anything that reads or writes user tables.
- Any Tauri, shell, or frontend change. The shell does not depend on the crate
  yet.
- The backup itself (the `backup` feature is only enabled here).

## Risks

- `rusqlite`'s MSRV moving past 1.90: pick the version accordingly.
- `libsqlite3-sys` must pick up `+crt-static` through `cc`. The dumpbin check
  proves it once the shell **links** the crate into the exe, which happens in
  M10. M4 only adds the shell dependency for `#[cfg(test)]` type exports,
  which does not link SQLite into the release exe. If the Windows job fails on
  CRT imports then, the fix is in the build configuration, not by dropping
  the check.
- `is_readonly` and the read-only retry must not mask other open failures:
  only `SQLITE_READONLY`/`SQLITE_CANTOPEN` on an existing file retry.
- The folder probe briefly creates a hidden temp file beside the database. It
  is removed on drop. A crash between create and drop could leave a
  `.fm-dbmanager-probe-*` file behind; that is accepted, because the
  alternative (reading Windows ACLs) is far more complex and less reliable.

## Review round 1

The reviewer requested changes on the first implementation. Fix all of the
following in one run; the rules above already describe the target behaviour.

**R1 — `busy_timeout` before the probe** (rule 3).

- Move `conn.busy_timeout(3000 ms)` to directly after each open (including
  the read-only reopen), before the `sqlite_schema` probe.
- Add `OpenError::Busy(PathBuf)`. `SQLITE_BUSY`/`SQLITE_LOCKED` from the
  probe after the timeout map to it, never to `Io`.
- Test `open_busy_waits_then_reports_busy`, in a temp dir:
  1. create a database in rollback-journal mode (`journal_mode = DELETE`);
  2. from a second connection, run `BEGIN EXCLUSIVE` and a write;
  3. call `Database::open` with a short timeout made injectable for tests (for
     example `pub(crate) fn open_with_timeout`) set to about 200 ms;
  4. it returns `OpenError::Busy` after at least that long.

  Test `open_after_lock_released_succeeds`: release the lock and open again
  successfully.

**R2 — read-only folder** (rule 4).

- Implement the folder probe and the two distinct reasons.
- Add `open_readonly_folder_falls_back_to_read_only` (Unix: chmod the temp
  folder to `0o555`; skip with a printed note when running as root, and on
  Windows, where the test cannot set an ACL cheaply):
  - `read_only_reason()` names the folder;
  - the folder contains no leftover probe file afterwards;
  - a writable folder leaves no probe file either
    (`open_probe_leaves_nothing_behind`).
- `tempfile` becomes a normal dependency of `fm-dbmanager` (workspace
  version).

**R3 — `dead_code` attribute** in `crates/fm-core/src/dirs.rs` (around line
83). Replace
`#[cfg_attr(not(windows), allow(dead_code))]` with exactly:

```rust
#[cfg_attr(all(not(windows), not(test)), expect(dead_code, reason = "used only by the Windows branch of system_config_dir and by tests"))]
```

The reviewer verified that this form is clean in both the lib and the test
builds; `cfg_attr(not(windows), expect(...))` fails in test builds, because
the tests use the function. Never use a bare `allow`.

**R4 — case-insensitive recent paths** in `state.rs` `same()`.

- On Windows, compare the ASCII-case-folded `to_str()` forms.
- If either path is not valid Unicode, fall back to `a == b`.
- Add a `#[cfg(windows)]` test `recent_dedupes_case_insensitively_on_windows`:
  touching `C:\A.db` then `c:\a.db` leaves one entry, the later spelling.

**R5 — tests.**

- `recent_touch_orders_dedupes_and_caps` asserts the **full** list order after
  de-duplication, not just its length or first element.
- Add the `#[cfg(unix)]` test `recent_touch_ignores_non_unicode`: a path built
  with `std::os::unix::ffi::OsStrExt::from_bytes(b"/tmp/\xff.db")` leaves the
  list unchanged.
- In `open_readonly_file_falls_back_to_read_only` (and the new folder test),
  restore permissions through a guard whose `Drop` resets them, so a failing
  assertion never leaves an undeletable temp dir.

**R6 — documentation of where SQLite is linked.** The Risks entry above now
says M10 links the crate (M4 only adds a test-only dependency). Check that no
comment or doc you wrote says otherwise.

**R7 — `CLAUDE.md` formatting.** Rewrap the new Status sentence, the edited
forbidden-DLL line and the `-p fm-dbmanager` line to the file's existing wrap
width (about 79 columns, matching the surrounding text). Change only the
wrapping; the content stays the same.

### Acceptance criteria (review round 1)

- `open_busy_waits_then_reports_busy`, `open_after_lock_released_succeeds`,
  `open_readonly_folder_falls_back_to_read_only` and
  `open_probe_leaves_nothing_behind` pass.
- `recent_touch_ignores_non_unicode` passes on Unix, and
  `recent_dedupes_case_insensitively_on_windows` on the Windows CI job.
- `open_readonly_file_falls_back_to_read_only` uses a permission-restoring
  guard; the read-only reasons distinguish file and folder.
- `crates/fm-core/src/dirs.rs` contains the exact `cfg_attr(... expect(...))`
  above and no `allow(dead_code)`.
- `cargo clippy --workspace --all-targets -- -D warnings` is clean in both
  normal and test builds on Linux, and on the Windows CI job.
- `CLAUDE.md` lines touched by M1 wrap at the file's width.
- `cargo fmt --all --check` and `cargo test --workspace` pass.
- The diff goes back to the reviewer.
