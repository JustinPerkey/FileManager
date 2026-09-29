# M4 — Archive writer

Status: awaiting approval
Project: tarpack   Depends on: M2 (landed), M3 (landed)

## Goal

Given a validated manifest and a Windows source file for every entry, write a
correct `.tar` atomically, verify it, and report a summary.

## Context

What earlier tasks provide:

- **M3** provides `fm_tarpack::manifest`. `Manifest` and `Entry` are already
  validated, with defaults resolved: target dir, target name, mode, uid, gid,
  uname, gname, plus the manifest's `dir_mode` and default owner. The format and
  its meaning are documented in `docs/tarpack-manifest.md`. Read it.
- **M2** provides `fm_core::atomic_write` and `FmError`.

**Assignments.** M5 builds the real drop and pick logic in parallel with this
task. Define the minimal input type here, in
`crates/fm-tarpack/src/sources/assignments.rs`:
`Assignments(BTreeMap<String /* entry id */, PathBuf>)`. Give it `new`,
`insert`, `get`, and `remove`, and nothing more. M5 extends this same type; it
does not replace it.

### Archive semantics

These are fixed decisions; implement them exactly.

- **Order.** Entries go in manifest order. Before each file, emit a directory
  entry for every ancestor of its target directory that has not been emitted
  yet, shallowest first. Each directory is emitted once.
- **Directory entries** have mode `dir_mode`, the default uid, gid, uname, and
  gname, and an mtime equal to the newest source mtime in the archive.
- **Paths** are relative: strip the leading `/`. Directory entries end in `/`.
  Extracting with `tar -xpf x.tar -C /` must reproduce the absolute layout.
- **Header format** is GNU (`tar::Header::new_gnu`), so paths longer than 100
  bytes work through the long-name extension.
- **File entries** take their mode, uid, gid, uname, and gname from the manifest
  entry, **never** from the Windows file, which has no Unix mode. `mtime` is the
  source file's modification time in whole seconds. The size and bytes are
  copied verbatim, with no line-ending conversion.

**Rules that bind this task.**

- Destructive operations are never silent. An existing output file is replaced
  only when the caller passes `overwrite = true`. A failure part way through
  leaves no output file and no temp file, and names the entry that failed.
- Tests never touch real user files; everything happens in a `TempDir`.
- The crate has no tauri dependency.
- Types that cross to the UI derive `ts_rs::TS`.

## Files

- `crates/fm-tarpack/src/archive/{mod.rs, plan.rs, write.rs, verify.rs}`
- `crates/fm-tarpack/src/sources/{mod.rs, assignments.rs}`: the minimal type
  above
- `crates/fm-tarpack/Cargo.toml`: adds `tar` and `sha2`; `tempfile` is already
  present or allowed
- Regenerated TS types

Do not change the manifest module except to add getters you need.

## Design

- **`ArchivePlan::new(&Manifest, &Assignments) -> Result<ArchivePlan, PlanError>`**
  - A pure function returning the ordered list of `PlannedEntry::Dir { path }`
    and `PlannedEntry::File { id, source, path, mode, owner }`.
  - `PlanError::Unassigned(Vec<id>)` lists every entry without a source.
- **`write_archive(plan, out_path, overwrite, progress: impl FnMut(Progress)) -> Result<BuildSummary, BuildError>`**
  1. If `out_path` exists and `overwrite` is false, return
     `BuildError::OutputExists` before touching anything.
  2. Stat every source first, and fail fast with `SourceMissing { id, path }` or
     `SourceUnreadable { id, path, cause }`.
  3. Stream into a `NamedTempFile` in `out_path`'s directory. Call `progress`
     with `{ entry_id, bytes_done, bytes_total }` at least once per entry.
  4. Finish the archive, flush it, and sync it.
  5. **Verify.** Re-open the temp file with `tar::Archive` and check that the
     entry sequence, paths, modes (`& 0o7777`), uid, gid, uname, gname, and
     sizes equal the plan. On a mismatch, return
     `BuildError::VerifyFailed(detail)`.
  6. Persist the temp file over `out_path`, and compute the SHA-256 of the
     final file.
  7. Return `BuildSummary { path, entries, files, dirs, bytes, sha256_hex }`.

## Acceptance criteria

- The archive round-trips exact paths, modes, owners, mtimes, sizes, and bytes.
- Parent directories appear once, before their children, with `dir_mode`.
- A missing or unreadable source, or an injected mid-write failure, leaves no
  output file and no temp file, and the error names the entry.
- `overwrite = false` never modifies an existing file.
- A mode the Windows source might suggest never leaks into the header.

## Tests proving completion

`cargo test -p fm-tarpack archive`. All tests run in a `TempDir`, with fixture
manifests built through M3's `parse`:

- `round_trip_preserves_headers`
- `parent_dirs_emitted_once_in_order`
- `shared_parent_dirs_not_duplicated`
- `long_paths_use_gnu_headers`
- `unassigned_entries_listed`
- `missing_source_writes_nothing`
- `existing_output_requires_overwrite`
- `overwrite_replaces_atomically`
- `summary_hash_matches_file`
- `modes_come_from_manifest_not_source`
- `extracts_with_system_tar_preserving_modes`: marked `#[cfg(unix)]`. It runs
  `tar -xpf <out> -C <tmp>` and checks the resulting `stat` modes.

Also run `cargo clippy -p fm-tarpack --all-targets -- -D warnings`.

## Out of scope

- Drop matching and remembered locations (M5).
- Tauri commands and threading (M6).
- Compression; plain `.tar` only.

## Risks

- `tar::Builder` writes mtime as seconds; truncate consistently in both the
  write and the verify step.
- On Windows, the temp file must be closed before `persist`.
