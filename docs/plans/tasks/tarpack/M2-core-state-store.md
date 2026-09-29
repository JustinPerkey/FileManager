# M2 — `fm-core`: app directories and the state store

Status: awaiting approval
Project: tarpack   Depends on: M1 (landed)

## Goal

Give every tool a namespaced, versioned, crash-safe place on disk to keep its
state.

## Context

`crates/fm-core` exists (from M1) as an empty library with no tauri dependency.
It is the shared layer for every tool. Nothing in it may know about a specific
tool.

The first consumer is the Tar Packager, which will store the last-used Windows
path for each manifest entry and a recent-manifest list. Your API must not
assume that shape; it stores any `serde` type.

The app runs on Windows, where app data lives under `%APPDATA%\FileManager\`.
The crate must also build and pass its tests on Linux.

**Rules that bind this task.**

- Tests never touch real user files or the real app-data directory. Every test
  runs inside a `tempfile::TempDir` it creates.
- Destructive operations are never silent. An unreadable state file is kept,
  not overwritten, and the caller is told.
- Paths stay as `PathBuf`. Do not round-trip them through `String`.

## Files

Create or edit `crates/fm-core/src/{lib.rs, dirs.rs, store.rs, atomic.rs,
error.rs}` and `crates/fm-core/Cargo.toml`. Allowed new dependencies:

- `directories`
- `serde`, `serde_json`
- `tempfile`, for the atomic write and as a dev dependency
- `thiserror`
- `time` or `chrono`, for the timestamp

Do not touch any other crate or the frontend.

## Design

- **`AppDirs`**
  - `AppDirs::from_system()` resolves the per-user app data directory
    `FileManager` through `directories`.
  - `AppDirs::at(root: PathBuf)` uses an explicit root. Tests use this.
  - `tool_dir(tool: &str) -> PathBuf` returns `<root>/<tool>/`. `tool` must
    match `[a-z][a-z0-9-]*`; anything else is an error.
- **`atomic_write(path, bytes)`**
  - Writes to a temp file in the same directory, flushes and syncs it, then
    renames it over the target.
  - On any error, no temp file is left behind.
- **`Store<T: Serialize + DeserializeOwned + Default>`**
  - `Store::load(&AppDirs, tool, name) -> Result<(Store<T>, Option<StoreWarning>)>`
  - `store.get() -> &T`
  - `store.update(|t| ...)` and `store.save()`
  - The file lives at `<root>/<tool>/<name>.json` and contains
    `{ "schema_version": N, "data": ... }`. The schema version is an associated
    const supplied by the caller.
- **Load outcomes**
  - Missing file: default state, no warning.
  - Corrupt JSON, or `schema_version` greater than the caller's: rename the
    file to `<name>.bad-<UTC timestamp>.json`, use the default state, and
    return `StoreWarning` naming the kept file.
  - Lower `schema_version`: the caller supplies a migration closure, or the
    same keep-and-reset path applies.
- **`FmError`**
  - A `thiserror` enum that is `Serialize`, so a later task can send it to the
    UI as `{ kind, message }`.

## Acceptance criteria

- Saving is atomic. A crash between write and rename leaves the old file
  intact.
- A corrupt or newer-version state file is preserved and reported, never
  silently overwritten.
- Two tools' stores cannot collide, and invalid tool names are rejected.
- Public items have doc comments.

## Tests proving completion

`cargo test -p fm-core`. Every test uses `TempDir` with `AppDirs::at`:

- `store_round_trips`
- `missing_file_gives_default_without_warning`
- `corrupt_state_is_preserved_and_reset`
- `newer_schema_is_not_overwritten`
- `namespaces_are_isolated`
- `invalid_tool_name_rejected`
- `atomic_write_leaves_no_partial_file_on_error`: inject a failure, for
  example a target whose parent is a file

Also run `cargo clippy -p fm-core --all-targets -- -D warnings`.

## Out of scope

- Anything tarpack-specific.
- Tauri wiring. M6 constructs `AppDirs::from_system()` in the shell.

## Risks

- Windows `rename` over an existing file must replace it. `tempfile`'s
  `persist` does this. Verify it on the Windows CI job.
