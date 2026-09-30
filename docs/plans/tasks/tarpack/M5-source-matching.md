# M5 — Source matching and remembered locations

Status: implemented and reviewer-approved at de7ae19 (original implementation
662099b, review follow-up R1–R8 88273bd, second-review fixes de7ae19)
Project: tarpack   Depends on: M2 (landed), M3 (landed, including its
partial-results follow-up). May run alongside M4.

## Goal

Assign Windows files to manifest entries, from drops (files or folders) or from
explicit picks, and remember each entry's last-used location, the last output
path, and the last chosen archive format per manifest across app restarts.

## Context

What earlier tasks provide:

- **M3:** `fm_tarpack::manifest`, with validated `Manifest` and `Entry`. Each
  entry has a stable `id` and a `source`, the expected Windows file name. The
  format is in `docs/tarpack-manifest.md`.
- **M3, partial results.** `manifest::parse(text)` returns a `ParseReport`;
  build test fixtures from `report.manifest` after asserting
  `report.is_valid()`. A manifest file with errors still yields a `Manifest`
  that holds only the entries that passed (`is_complete()` is `false`). M6
  calls this task's matching and restore functions on such a manifest too,
  so the user can assign files while fixing the manifest. The remembered
  sources of entries that failed must survive that: see `restore_all`
  below.
- **M3:** `fm_tarpack::format::ArchiveFormat { Tar, TarGz, TarZst, TarXz }`
  (serde, `ts_rs::TS`), and `Manifest::default_format()`, which gives the format
  implied by the manifest's `output_name` suffix, or `Tar`.
- **M2:** `fm_core::{AppDirs, Store}`. It is namespaced per tool, versioned, and
  atomic, and it reports corrupt files instead of overwriting them.
- **M4**, if it has landed, defined
  `fm_tarpack::sources::Assignments(BTreeMap<String, PathBuf>)` with `new`,
  `insert`, `get`, and `remove`. If it exists, extend that type, and keep its
  existing API. If M4 has not landed, create it with exactly that API at
  `crates/fm-tarpack/src/sources/assignments.rs`, so the two tasks converge.

**The user's workflow.** The user rebuilds the same package many times a day.
They drag a build output folder, or individual files, onto the window, or pick
a file for one row. The next time they open the same manifest, every row should
already point at the last file used, and the output format should be the one
they last chose for that manifest (the human's decision: remember the last
format per manifest, in the tool's state store).

**Rules that bind this task.**

- Never guess:
  - A dropped file whose name matches more than one unassigned entry is
    reported as ambiguous and assigned to none of them.
  - A file that matches no entry is reported as unmatched.
- Windows file names are case-insensitive, so name matching is too.
- Paths are kept as `PathBuf`/`OsString` and never lossily converted, except in
  a display string.
- Tests never touch real user files; build fixture trees in a `TempDir`.
- The crate has no tauri dependency.
- UI-facing types derive `ts_rs::TS` (via the workspace `ts-rs`
  dependency, which M3 added to this crate). Export them through the export
  test that M1 created:
  - the test lives at `apps/desktop/src-tauri/src/generated_types.rs`;
  - append one entry per root type to its `EXPORTERS` list,
    `<fm_tarpack::path::Type as ts_rs::TS>::export_all`;
  - regenerate with
    `UPDATE_GENERATED=1 cargo test -p filemanager --lib generated_types_are_current`
    (recorded in `CLAUDE.md`, with the PowerShell form);
  - never hand-edit, and never use `#[ts(export)]`.
- **Parallel with M4.** M4 also appends to `EXPORTERS` and
  regenerates `lib/generated/`. If M4 merges first, resolve any conflict
  in `EXPORTERS` by keeping both sets of entries. Then re-run the regenerate
  command. Never hand-merge generated files. If you merge first, M4 does
  the same.
- No `bigint` crosses the boundary: any 64-bit integer field in an exported
  type carries `#[ts(type = "number")]`. `DropOutcome` currently has none.

## Files

- `crates/fm-tarpack/src/sources/{mod.rs, assignments.rs, matching.rs, remembered.rs}`
- `apps/desktop/src-tauri/src/generated_types.rs`: append the exporters for
  the UI-facing types this task adds (`EntryStatus`, `DropOutcome`). Change
  nothing else in the file.
- Regenerated TS types in `apps/desktop/src/lib/generated/`, written by the
  regenerate command

## Design

- **Status.** `EntryStatus::{Ready, Missing, Unassigned}`. `Missing` means an
  assigned path that no longer exists or is not a file.
  `Assignments::status(&self, id) -> EntryStatus` checks the filesystem at call
  time.
- **Drops.**
  `match_dropped(&Manifest, &Assignments, &[PathBuf]) -> DropOutcome { matched: Vec<(id, PathBuf)>, unmatched: Vec<Unmatched>, ambiguous: Vec<Ambiguity> }`,
  where `Unmatched { path: PathBuf, reason: UnmatchedReason }`. (An earlier
  draft of this line said `unmatched: Vec<PathBuf>`, which contradicted the
  reason below; the typed reason is specified in "Review follow-up", R2.)
  - A dropped **file** matches entries whose `source` equals its file name,
    compared case-insensitively. Candidates are only entries that are
    unassigned or `Missing`. A drop does not silently replace a `Ready`
    assignment; that case is reported in `unmatched`, with the reason
    `AlreadyAssigned`.
  - A dropped **folder** is walked recursively, to a depth of at most 8. The
    walk does not follow symlinks or junctions. Each still-open entry is
    matched by name, and more than one candidate file for an entry is an
    `Ambiguity { id, candidates }`.
  - An entry that two dropped files both match is ambiguous.
  - `match_dropped` computes and applies nothing. `apply(&mut Assignments, &DropOutcome)`
    applies only `matched`.
- **Picks.** `Assignments::insert(id, path)` accepts any file regardless of its
  name, because the user decided.
- **Remembered state.**
  `RememberedState { per_manifest: BTreeMap<String /* canonical manifest path */, ManifestMemory>, recent_manifests: Vec<PathBuf> /* MRU, max 10 */ }`
  with `ManifestMemory { sources: BTreeMap<id, PathBuf>, last_output: Option<PathBuf>, last_format: Option<ArchiveFormat> }`.
  Mark `last_format` (and every `Option` field) `#[serde(default)]`, so a state
  file without it still loads.
  - It is persisted through `Store::<RememberedState>::load(dirs, "tarpack", "state")`
    with `schema_version = 1`.
  - `remember(&mut self, manifest_path, &Assignments, output: Option<&Path>, format: ArchiveFormat)`
    stores all three.
  - `restore(&self, manifest_path, &Manifest) -> Assignments`. Ids no longer in
    the manifest are dropped. The next `remember` prunes them from state.
  - `restore_all(&self, manifest_path) -> Assignments` returns every
    remembered source for that manifest, unfiltered. M6 uses it instead of
    `restore` when the manifest has errors, so that an entry which failed
    validation (its id is not among the passed entries) keeps its remembered
    file through the next `remember`: a typo in one entry must not erase
    that entry's remembered location.
  - `restore_output(&self, manifest_path) -> Option<PathBuf>` returns the
    remembered last output.
  - `restore_format(&self, manifest_path, &Manifest) -> ArchiveFormat` returns
    the remembered `last_format`, or `manifest.default_format()` when none is
    remembered.
  - `touch_recent(path)` moves the path to the front and caps the list at 10.
  - The key is the canonicalized path. On Windows, strip the `\\?\` prefix and
    lowercase it.

## Acceptance criteria

- Each `DropOutcome` bucket is populated correctly, including the
  `AlreadyAssigned` case.
- A folder drop finds nested files and never follows links.
- Assignments survive a restart: a new `Store` over the same temp root restores
  them, with `Missing` status for deleted files.
- Removed ids are pruned.
- The recent list is MRU-ordered and capped at 10.
- The last format survives a restart per manifest. With nothing remembered, the
  format falls back to the manifest's `output_name` suffix, then `Tar`. Two
  manifests remember independent formats.

## Tests proving completion

`cargo test -p fm-tarpack sources`, all in a `TempDir`:

- `drop_matches_case_insensitively`
- `drop_does_not_replace_ready_assignment`
- `folder_drop_finds_nested_files`
- `folder_drop_respects_depth_cap`
- `ambiguous_drop_is_not_assigned`
- `unmatched_files_reported`
- `pick_accepts_any_name`
- `remembered_locations_restore_with_missing_status`
- `removed_ids_are_pruned`
- `restore_all_keeps_ids_not_in_manifest`: remember sources for `a` and `b`,
  then `restore_all` returns both while `restore` against a manifest with
  only `a` returns only `a`; `remember` with the `restore_all` result keeps
  `b` across a restart
- `recent_manifests_mru_capped`
- `last_format_restored_per_manifest`
- `format_falls_back_to_output_name_then_tar`
- `state_without_last_format_loads`: a hand-written v1 JSON without the field

`symlinks_not_followed` is marked `#[cfg(unix)]`.

Also run `cargo test -p filemanager generated_types_are_current` and
`cargo clippy --workspace --all-targets -- -D warnings`.

## Out of scope

- Writing archives or choosing encoders (M4).
- Rewriting output-path extensions when the format changes (M6).
- Tauri commands, file watching, and dialogs (M6).

## Risks

- Keying by canonical path means renaming or moving a manifest forgets its
  remembered locations. This is accepted; mention it in
  `docs/tarpack-manifest.md` under a "Remembered locations" note.

## Review follow-up

M5 was implemented in the working tree (uncommitted):
`crates/fm-tarpack/src/sources/{mod.rs, assignments.rs, matching.rs, remembered.rs, tests.rs}`,
the two new `EXPORTERS` lines in `apps/desktop/src-tauri/src/generated_types.rs`,
the generated `Ambiguity.ts`, `DropOutcome.ts`, `EntryStatus.ts`, and
`Unmatched.ts` in `apps/desktop/src/lib/generated/`, and a "Remembered
locations" note in `docs/tarpack-manifest.md`. The reviewer requested
changes. This section is the complete brief for the rework run: do R1–R8 on
top of that working tree, keep everything above that R1–R8 does not change,
then hand the whole M5 diff to the reviewer.

### Accepted as implemented (do not change)

The reviewer accepted these choices. They are now part of the spec:

- **Depth cap.** The dropped folder's direct children are level 1. A file at
  level 8 (`MAX_DEPTH`) is found; one at level 9 is not. Directories below
  the cap are not entered and not reported.
- **`remember` replaces** the stored sources for that manifest with the
  given `Assignments`. That replacement is how removed ids are pruned. M6
  passes `restore_all`'s result while the manifest has errors, so failed
  entries survive.
- **Key fallback.** If `canonicalize` fails (the manifest was deleted or is
  unreachable), the key is built from the path as given, with the same
  Windows normalisation (R1).
- **Links.** The walk skips any entry whose `file_type().is_symlink()` is
  true. On Windows this covers junctions as well as symlinks, so no
  separate junction check is needed.

### R1 (P1). The state-key test must use the real key function

`tests.rs` `state_without_last_format_loads` builds the JSON key with
`canonicalize().to_string_lossy()`. `remembered.rs` `key()` strips `\\?\` and
lowercases on Windows, so the hand-written key never matches there, and the
test fails on the Windows CI job.

- Make `key` `pub(super)` in `remembered.rs` (visible to `sources::tests`,
  not outside `sources`). After R3 its signature is
  `fn key(path: &Path) -> Option<String>`.
- The test derives the key through `super::remembered::key(&mpath).unwrap()`
  and escapes it for JSON. Do not duplicate the normalisation in the test.
- Windows normalisation, stated exactly: canonicalize (or fall back to the
  path as given); convert with `to_str()` (R3); if the string starts with
  `\\?\UNC\`, replace that prefix with `\\`; else if it starts with `\\?\`,
  remove it; then `to_lowercase()`. On other platforms the key is the
  canonical path's `to_str()` unchanged.
- New test `#[cfg(windows)] key_ignores_case_and_verbatim_prefix`: in a
  `TempDir`, create `m.toml`. Let `plain` be the path as created, `upper` the
  same path with its string uppercased, and `verbatim` the result of
  `plain.canonicalize()` (which carries `\\?\`). `remember` through `plain`,
  then `restore_all` through `upper` and through `verbatim` returns the same
  source; `remember` through each of the three leaves
  `per_manifest.len() == 1`; `touch_recent` through all three leaves one
  entry in `recent_manifests`.

### R2 (P2). A typed unmatched reason

`Unmatched.reason` is free English text today. Replace it with an enum, so
the UI maps each reason to its own copy and a new reason fails the TS
typecheck instead of showing raw text.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum UnmatchedReason {
    AlreadyAssigned, // name fits only entries that are Ready; the drop never replaces them
    NoEntry,         // a directly dropped file whose name fits no passed entry
    NotFound,        // a dropped path that does not exist (io::ErrorKind::NotFound)
    LinkNotFollowed, // a dropped path that is a symlink or junction
    FolderNoMatch,   // a dropped folder in which no file's name fits any passed entry
    Unreadable,      // a dropped path, or a directory or child met in the walk, that could not be read
    NotUnicode,      // a file that would be assigned, but its path is not valid Unicode (R3)
}

pub struct Unmatched { pub path: PathBuf, pub reason: UnmatchedReason }
pub struct DropOutcome {
    pub matched: Vec<(String, PathBuf)>,
    pub unmatched: Vec<Unmatched>,
    pub ambiguous: Vec<Ambiguity>,
}
```

- Generated TS: `UnmatchedReason = "alreadyAssigned" | "noEntry" | "notFound" | "linkNotFollowed" | "folderNoMatch" | "unreadable" | "notUnicode"`,
  `Unmatched = { path: string, reason: UnmatchedReason }`, and
  `DropOutcome.unmatched: Array<Unmatched>`.
- Doc-comment each variant (ts-rs carries the comments into the TS).
- Export: `DropOutcome`'s `export_all` already writes the types it depends
  on, so `UnmatchedReason.ts` appears without a new `EXPORTERS` line. Check
  that it does; add a line only if it does not. Regenerate with
  `UPDATE_GENERATED=1 cargo test -p filemanager --lib generated_types_are_current`;
  never hand-edit `lib/generated/`.
- `symlink_metadata` on a dropped path: `NotFound` only for
  `io::ErrorKind::NotFound`; any other error is `Unreadable`.
- Re-export `UnmatchedReason` from `sources/mod.rs` next to `Unmatched`.

### R3 (P2). Paths that are not valid Unicode: an accepted limit

serde serializes a `PathBuf` only when it is valid Unicode. Today one such
path in `RememberedState` makes every later `Store::save` fail, one in a
`DropOutcome` makes `tarpack_assign_dropped` fail over IPC as a whole, and
`key()`'s `to_string_lossy` lets two different manifest paths share a key.
The policy (project decision) is: **such paths are rejected where they enter
through a drop, reported with `NotUnicode`, and never stored or sent as a
path.** No lossless encoding is added. In the app they can only arise from a
folder walk, because every path the UI sends (drops, Browse, Save) arrives as
a JSON string and is therefore already Unicode. Exactly:

- **Matching.**
  - `matched` holds only paths whose `to_str()` is `Some`. When the rules
    would assign a file (an entry's single, unshared candidate) whose path
    is not Unicode, the entry is left unassigned and the file goes to
    `unmatched` with `NotUnicode`.
  - A directly dropped file whose file name is not Unicode cannot be
    compared with a `source`: report it as `NotUnicode`, not `NoEntry`.
    A walked file whose name is not Unicode is ignored, like any other
    unrelated file in a folder.
  - A candidate that is not Unicode still counts when deciding ambiguity (so
    it cannot turn a real ambiguity into a guess) and stays in that
    `Ambiguity.candidates`.
- **Display-only paths in the outcome.** `Unmatched.path` and
  `Ambiguity.candidates` are shown to the user and never sent back. Serialize
  them lossily (`to_string_lossy`) through two serde `with` modules in
  `matching.rs`, `lossy` and `lossy_vec`, so no path in them can fail
  serialization. The fields are annotated
  `#[serde(with = "lossy")] #[ts(type = "string")]` (`Unmatched.path`) and
  `#[serde(with = "lossy_vec")] #[ts(type = "Array<string>")]`
  (`Ambiguity.candidates`); the explicit `#[ts(type)]` keeps the generated
  shapes unchanged. The `#[ts(type)]` override is used instead of ts-rs's
  `no-serde-warnings` feature, because Cargo features unify workspace-wide
  and that feature would silence the serde/TS mismatch warning for every
  crate. They stay `PathBuf` in Rust, so tests compare them exactly.
  `DropOutcome`, `Unmatched`, `Ambiguity`, and `UnmatchedReason` derive
  `Serialize` only (no `Deserialize`): they are outbound, and a lossy field
  must not be read back as a path. Their doc comments say so.

  A path dropped more than once is reported once, and a walk error reached
  through overlapping drops (a folder and a folder inside it) is reported
  once. The dedup is by exact path, so it does not fold Windows case variants
  of the same path. That is accepted: OS drops carry the file system's real
  spelling, so case variants do not arise in practice.
- **Remembered state never holds such a path.**
  - `key(path) -> Option<String>` uses `to_str()`, never `to_string_lossy`.
    `None` means the manifest has no memory: `remember` does nothing,
    `restore` and `restore_all` return empty `Assignments`,
    `restore_output` returns `None`, and `restore_format` returns
    `manifest.default_format()`.
  - `remember` skips any source whose path is not Unicode (that id is not
    stored) and stores `last_output: None` when the output path is not
    Unicode.
  - `touch_recent` ignores a path that is not Unicode.
  - The doc comments of `remember` and `touch_recent` state the skip, and
    that it is defence in depth: the app's inputs are already Unicode, and
    the drop boundary reports `NotUnicode`.
- `Assignments::insert` keeps M4's API and accepts any path, because the
  writer can read any file. Only persistence and IPC are limited.
- Note in `docs/tarpack-manifest.md`, under "Remembered locations": a file or
  folder whose path is not valid Unicode cannot be assigned by dropping it;
  the drop result says so. Rename it.

### R4 (P3). A folder whose matches are all already assigned

`match_dropped` (matching.rs, the `useful` check) treats a folder as useful
when any file's name fits any entry, `Ready` ones included, but the walked
files that fit only `Ready` entries are then dropped without a report. The
user sees an empty result. New rules for walked files:

- A walked file whose name fits one or more entries, all `Ready`, is
  reported as `AlreadyAssigned`, the same as a direct drop.
- `FolderNoMatch` is reported for a dropped folder only when no file found
  in it has a name that fits any passed entry (open or `Ready`), and the
  folder itself was readable.
- Walked files that fit no entry are still not reported.

### R5 (P3). Unreadable directories are reported

`walk` skips `read_dir` and `file_type` errors silently (matching.rs, `let
Ok(...) else`), which can hide a second candidate and turn an ambiguity into
a guess.

- `walk` reports each directory whose `read_dir` fails, each iteration
  item that is an `Err`, and each child whose `file_type()` fails, as
  `Unreadable` in `unmatched` (path: that directory, or the child's path;
  for an `Err` item with no path, the directory being read), and continues
  with the rest.
- A dropped folder that cannot be read at all gives one `Unreadable` for it,
  and no `FolderNoMatch`.
- Matches elsewhere still apply. The report makes the gap visible; blocking
  every match because one subfolder (for example a system folder) is
  unreadable would make folder drops unusable.

### R6 (P3). A file dropped directly keeps its direct status

The dedup (matching.rs, `seen`) keeps the first occurrence, so a file that is
dropped directly and also found inside a dropped folder loses its "direct"
flag when the folder comes first, and with it its `NoEntry` or
`AlreadyAssigned` report. Merge instead: one record per path, whose `direct`
is true if any occurrence was direct. A path dropped twice is processed and
reported once.

### R7 (P3). Tests

All in a `TempDir`, in `crates/fm-tarpack/src/sources/tests.rs`. Add
`serde_json = "1"` (the version `fm-core` uses) under
`[dev-dependencies]` in `crates/fm-tarpack/Cargo.toml` for the serialization
tests.

Strengthen:

- `pick_accepts_any_name`, `apply` part: build an outcome from
  `match_dropped` that has one matched, one ambiguous, and one unmatched
  item. After `apply` on empty `Assignments`, only the matched id is
  assigned; the ambiguous ids are unassigned, and no assignment points at
  the unmatched path or at any ambiguous candidate.
- `unmatched_files_reported`: assert each `(path, reason)`, not only the
  paths: `NoEntry`, `NotFound`, and `FolderNoMatch` at least.
- `drop_does_not_replace_ready_assignment`: compare with
  `UnmatchedReason::AlreadyAssigned`.
- `symlinks_not_followed`: the directly dropped link has `LinkNotFollowed`.
- `removed_ids_are_pruned`: go through a restart. `remember` the full set,
  `save`, load a new `Store` over the same temp root, `restore` against
  the manifest without `b`, `remember` that, `save`, load again, and assert
  that `restore_all` has no `b`.
- `state_without_last_format_loads`: key through `key()` (R1).

Add:

- `shared_candidate_is_ambiguous`: entries `a` and `b` both with source
  `x.bin`, both open; drop a folder holding one `x.bin` → both ids are in
  `ambiguous`, each with that one candidate, and `matched` is empty. Then
  make `a` `Ready` and drop the same file → `b` is matched (a `Ready` entry
  is not a candidate, so nothing is shared).
- `folder_with_only_ready_matches_reports_already_assigned` (R4): every
  entry `Ready`; drop a folder holding files with those names → each is
  `AlreadyAssigned`, no `FolderNoMatch`, nothing matched.
- `direct_drop_keeps_status_when_folder_comes_first` (R6): drop `[F,
  F/notes.txt]` where `notes.txt` fits no entry → exactly one `NoEntry`
  for `F/notes.txt`; and a file of `F` that fits a `Ready` entry, dropped
  as `[F, F/a.bin]`, is reported once as `AlreadyAssigned`.
- `#[cfg(unix)] unreadable_directory_reported` (R5): a subdirectory with
  mode `0o000` gives one `Unreadable` with its path, and a matching file
  elsewhere in the folder is still matched. If `read_dir` on it still
  succeeds (the test runs as root), return early without asserting. Restore
  the mode before the `TempDir` is dropped.
- `unmatched_reason_serializes_camel_case`: `serde_json::to_value` of each
  variant gives the strings listed in R2.
- `non_unicode_match_is_reported` (R3), `#[cfg(any(target_os = "linux", windows))]`:
  a folder `<bad>/a.bin`, where `<bad>` is `OsStr::from_bytes(b"\xff")` on
  Linux and `OsString::from_wide(&[0xD800])` on Windows (cfg'd helper, no
  `unsafe`). Drop the parent folder → `matched` is empty, `unmatched` holds
  that file with `NotUnicode`, and `serde_json::to_string(&outcome)` is
  `Ok`.
- `outcome_serializes_display_paths_lossily` (R3), same cfg: an outcome
  with a non-Unicode `Unmatched.path` and `Ambiguity` candidate serializes,
  and the JSON strings contain U+FFFD.
- `non_unicode_paths_are_not_remembered` (R3), same cfg: `remember` with
  one Unicode and one non-Unicode source and a non-Unicode output, then
  `touch_recent` with a non-Unicode manifest path, then `save` → `Ok`.
  After a restart the Unicode id is restored, the other is absent,
  `restore_output` is `None`, and `recent_manifests` does not contain the
  bad path. `remember` through a non-Unicode manifest path leaves
  `per_manifest` unchanged.
- `#[cfg(windows)] key_ignores_case_and_verbatim_prefix` (R1).

### R8. Documentation

- `CLAUDE.md`, the layout line for `crates/fm-tarpack/src/sources/`: it now
  reads only "Assignments: entry id -> Windows source file". Replace it with
  a line (wrapped like its neighbours) that also names drop matching
  (`match_dropped`, `apply`) and remembered state (`RememberedState`,
  persisted as `tarpack/state` in `fm-core`'s `Store`). Change nothing else
  in `CLAUDE.md`.
- `docs/tarpack-manifest.md`: the R3 sentence under "Remembered locations".

### Follow-up acceptance

- `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace` pass, including
  `generated_types_are_current` (so `lib/generated/` is regenerated, with
  the new `UnmatchedReason.ts`).
- `cargo test -p fm-tarpack sources` runs every test named in R7 and above.
- No path in `matched`, and none in `RememberedState`, can be non-Unicode;
  a `DropOutcome` always serializes.
- No `unmatched` reason is a string anywhere in Rust.
- `npm run typecheck` still passes (no TS outside `lib/generated/` reads
  `Unmatched.reason` yet; the UI consumes it in U4).
