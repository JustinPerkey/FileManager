# M5 — Source matching and remembered locations

Status: awaiting approval
Project: tarpack   Depends on: M2 (landed), M3 (landed). May run alongside M4.

## Goal

Assign Windows files to manifest entries, from drops (files or folders) or from
explicit picks, and remember each entry's last-used location, the last output
path, and the last chosen archive format per manifest across app restarts.

## Context

What earlier tasks provide:

- **M3:** `fm_tarpack::manifest`, with validated `Manifest` and `Entry`. Each
  entry has a stable `id` and a `source`, the expected Windows file name. The
  format is in `docs/tarpack-manifest.md`.
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
- UI-facing types derive `ts_rs::TS`.

## Files

- `crates/fm-tarpack/src/sources/{mod.rs, assignments.rs, matching.rs, remembered.rs}`
- Regenerated TS types

## Design

- **Status.** `EntryStatus::{Ready, Missing, Unassigned}`. `Missing` means an
  assigned path that no longer exists or is not a file.
  `Assignments::status(&self, id) -> EntryStatus` checks the filesystem at call
  time.
- **Drops.**
  `match_dropped(&Manifest, &Assignments, &[PathBuf]) -> DropOutcome { matched: Vec<(id, PathBuf)>, unmatched: Vec<PathBuf>, ambiguous: Vec<Ambiguity> }`
  - A dropped **file** matches entries whose `source` equals its file name,
    compared case-insensitively. Candidates are only entries that are
    unassigned or `Missing`. A drop does not silently replace a `Ready`
    assignment; that case is reported in `unmatched`, with the reason
    "already assigned".
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
  "already assigned" case.
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
- `recent_manifests_mru_capped`
- `last_format_restored_per_manifest`
- `format_falls_back_to_output_name_then_tar`
- `state_without_last_format_loads`: a hand-written v1 JSON without the field

`symlinks_not_followed` is marked `#[cfg(unix)]`.

Also run `cargo clippy -p fm-tarpack --all-targets -- -D warnings`.

## Out of scope

- Writing archives or choosing encoders (M4).
- Rewriting output-path extensions when the format changes (M6).
- Tauri commands, file watching, and dialogs (M6).

## Risks

- Keying by canonical path means renaming or moving a manifest forgets its
  remembered locations. This is accepted; mention it in
  `docs/tarpack-manifest.md` under a "Remembered locations" note.
