# M2b — Move the Tar Packager session logic into `fm-tarpack`; the shared `FileWatcher`

Status: awaiting approval
Project: gpui-migration   Depends on: M2a (landed)

## Goal

Move the Tar Packager's session logic, session types and progress
coalescer from the Tauri shell into `fm-tarpack::session`, and its manifest
watcher into `fm-core` as the shared `fm_core::watch::FileWatcher`, with all
their tests, leaving the Tauri commands as thin glue and
`apps/desktop/src/lib/generated/` byte-identical.

## Context

FileManager is moving its UI from Tauri + React to GPUI (a native Rust UI).
The new UI calls each tool's session logic directly, so it must live in the
tool's domain crate. M2a already did this for the Pipeline
(`crates/fm-pipeline/src/session/`) and the Schedule Creator
(`crates/fm-schedule/src/session/`): follow the same shape. A second tool
(the Database Manager) will need the file watcher too, and tools never import
each other, so the watcher goes into `fm-core`.

What exists in `apps/desktop/src-tauri/src/tools/tarpack/`:

- `core.rs`: `Core` (`new(&AppDirs)`, `unpersisted(String)`,
  `manifest_path`, `session`, `recent_manifests`, `open`, `reload`,
  `assign`, `clear`, `begin_drop`, `finish_drop`, `set_format`,
  `set_output`, `create_from_example`, `begin_build`, `finish_build`,
  `abort_build`, `manifest_path_for_open`, `reveal_target`, `snapshot`),
  `DropJob::run`, `BuildJob::run`, `with_format`, `write_new_file`, and
  `EXAMPLE_MANIFEST = include_str!("../../../../../../examples/tarpack/example.toml")`.
- `types.rs`: `SessionEntry`, `SessionManifest`, `ArchiveFormatOption`,
  `BuildBlockedReason`, `TarpackSession`, `DroppedAssignment`,
  `ManifestChanged`, `TarpackErrorKind`, `TarpackError` and its `From`
  impls for `fm_tarpack::BuildError`, `fm_tarpack::PlanError`,
  `fm_core::FmError`.
- `progress.rs`: `PROGRESS_INTERVAL` (50 ms), `ProgressCoalescer`,
  `coalescing`.
- `watch.rs`: `DEBOUNCE` (300 ms), `ManifestWatcher { start(path, on_change), path() }`,
  `is_manifest_change(events, name)`. It watches the file's **parent
  directory** non-recursively with `notify-debouncer-mini` (editors save by
  replacing the file) and calls `on_change` once per burst for that file
  name.
- `mod.rs`: the `tarpack_*` commands, `TarpackState { core, watcher }`,
  `arm_watcher`, the two event names.
- `tests.rs` (about 1,230 lines): drives `Core`, `DropJob`, the coalescer
  and the watcher in temp dirs.
- `apps/desktop/src-tauri/src/generated_types.rs` exports
  `crate::tools::tarpack::types::{TarpackSession, ArchiveFormatOption,
  TarpackError, TarpackErrorKind, DroppedAssignment, ManifestChanged}`.

### What to build

- `crates/fm-tarpack/src/session/{mod,core,types,progress,tests}.rs`: the
  code of the shell's `core.rs`, `types.rs`, `progress.rs` and the
  non-watcher parts of `tests.rs`, moved, not rewritten. `EXAMPLE_MANIFEST`
  points at `examples/tarpack/example.toml` from its new depth. What the
  shell calls becomes `pub`; helpers stay private (`with_format` and
  `write_new_file` may be `pub(crate)` if tests need them).
- `crates/fm-core/src/watch.rs`: `ManifestWatcher` renamed **`FileWatcher`**,
  same behaviour and debounce, same API:

  ```rust
  pub const DEBOUNCE: Duration = Duration::from_millis(300);
  pub struct FileWatcher { /* … */ }
  impl FileWatcher {
      /// Calls `on_change` once per burst of changes to the file at `path`
      /// (created, written, replaced, renamed into place, or deleted).
      pub fn start(path: &Path, on_change: impl Fn() + Send + 'static) -> Result<FileWatcher, String>;
      pub fn path(&self) -> &Path;
  }
  ```

  The doc comment says it watches the parent directory, never recursively.
  The watcher tests move with it into `fm-core` (rename
  `ManifestWatcher` → `FileWatcher` in them; keep their names otherwise).
  `fm-core/Cargo.toml` gains `notify-debouncer-mini` at the version the
  shell uses (`0.7.0`); move it to `[workspace.dependencies]` in the root
  `Cargo.toml` and use `{ workspace = true }` in both crates. Drop it from
  the shell if the shell no longer uses it directly.
- `tools/tarpack/mod.rs` imports from `fm_tarpack::session` and
  `fm_core::watch::FileWatcher` and keeps only the commands, `state()`,
  `arm_watcher` and the event names. Delete the shell's `core.rs`,
  `types.rs`, `progress.rs`, `watch.rs` and `tests.rs` (keep in the shell
  any test that exercises command glue itself, not `Core`).
- `generated_types.rs` names the six types at `fm_tarpack::session::…`;
  ts-rs attributes and doc comments copied exactly.

Rules that bind this task:

- **The `fm-*` crates never depend on tauri** (nor on GPUI). `fm-core` stays
  tool-agnostic: `FileWatcher` knows nothing about manifests.
- **Boundary types are defined once and mirrored:** `lib/generated/` must not
  change. Do not regenerate to make the test pass.
- Tools never import each other. Shared code goes in `fm-core`.
- Tests that touch the filesystem run in a temp dir they create.
- fmt clean; clippy `pedantic` with `-D warnings`; scoped `#[expect]` only.
  Library crates surface `must_use_candidate`, `missing_errors_doc` and
  `missing_panics_doc`: fix them.
- Paths stay `Path`/`PathBuf`; display strings only for display.

## Files

Create: `crates/fm-tarpack/src/session/{mod,core,types,progress,tests}.rs`,
`crates/fm-core/src/watch.rs`.

Edit: `Cargo.toml` (workspace dependency), `Cargo.lock`,
`crates/fm-core/{Cargo.toml,src/lib.rs}`, `crates/fm-tarpack/{Cargo.toml,src/lib.rs}`,
`apps/desktop/src-tauri/{Cargo.toml,src/tools/tarpack/mod.rs,src/generated_types.rs}`,
`CLAUDE.md` (layout: `fm-core` gains `watch.rs`; `fm-tarpack` gains
`src/session/`; the shell's `tools/tarpack/` line lists only `mod.rs`).

Delete: `apps/desktop/src-tauri/src/tools/tarpack/{core,types,progress,watch,tests}.rs`.

Do not touch: `apps/desktop/src/`, other tools, `fm-dbmanager`, `docs/plans/`.

## Acceptance criteria

- `git diff --stat main -- apps/desktop/src/lib/generated/` is empty and
  `generated_types_are_current` passes without regenerating.
- Every test of the shell's `tarpack/tests.rs` exists under the same name in
  `fm-tarpack`'s `session/tests.rs` or (watcher tests) in `fm-core`'s
  `watch.rs` tests, and passes there. Any rename is listed in the PR with its
  reason; none is dropped.
- `apps/desktop/src-tauri/src/tools/tarpack/` contains only `mod.rs`.
- The commands, the two events and their payloads behave as before; the
  frontend tests pass untouched.
- `fm-core` has no dependency on any `fm-*` tool crate; `cargo tree -i tauri`
  finds nothing for `fm-core` and `fm-tarpack`.
- fmt, clippy pedantic and `cargo test --workspace` pass on both CI jobs.

## Tests proving completion

- The moved tests (`cargo test -p fm-tarpack -p fm-core`), including the
  watcher tests and the coalescer tests.
- `generated_types_are_current`.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `npm run typecheck && npm run lint && npm run test`.

## Out of scope

- The watcher's "missing directory" behaviour (watching the deepest existing
  ancestor): a later Database Manager milestone adds it.
- Any GPUI code; any behaviour change; ts-rs removal.

## Risks

- The watcher tests are timing-based (debounce); keep their timeouts as they
  are. They must still pass on the Windows runner.
- ts-rs output is sensitive to doc comments and attributes: copy verbatim.
- `BuildJob::run` takes `impl FnMut(Progress)`; the shell's build command
  wraps it in `coalescing`. Keep that call in the shell.
