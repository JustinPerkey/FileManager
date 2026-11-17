# M13 — Config file watcher, automatic reload, and the developer-mode "Edit config"

Status: awaiting approval. New on 2026-11-16, from the human's final answers:
the config reloads automatically, and there is one fixed app-wide config.
It is numbered 13 so the milestone numbers the UI plans already cite stay
unchanged. **It runs after M10 and before the U task that shows the config
status and the developer-mode Edit config action.**
Project: dbmanager   Depends on: M10 (landed)

## Goal

Watch the app-wide Database Manager config file and reload it automatically
when it changes, and tell the UI with an event. Add the command behind
developer mode's **Edit config**, which opens the config in the system
editor and first creates a commented starter file if none exists. The file
watcher becomes shared code in `fm-core`, and the Tar Packager is moved onto
it.

## Context

The Database Manager (`dbmanager`) reads one config for the whole app. Its
path comes from `fm_core::system_config_dir()?.join("dbmanager.toml")`: on
Windows that is `%ProgramData%\FileManager\dbmanager.toml`, and on other
platforms `/etc/filemanager/dbmanager.toml`. The app only reads it. The
human decided:

- **Automatic reload.** When the file changes, the app reloads it and tells
  the UI. The UI keeps unsaved drafts and re-checks them against the new
  rules. A write prepared under the old config is refused once with
  `ConfigChangedOnDisk` and can be retried. That refusal already works: M10
  writes carry `configHash`, and `Core` reloads on a stale hash.
- **Edit config only in developer mode.** This is the same app-wide
  developer mode the Tar Packager uses (Ctrl+Alt+Shift+D, `useDevMode()` in
  `apps/desktop/src/app/devMode.tsx`). Gating it is the UI's job, exactly as
  tarpack's "Edit in editor" is gated in the views. The backend command, like
  `tarpack_open_in_editor`, is always callable.
- **If the file is missing,** the developer command creates a commented
  starter file. It never overwrites an existing file.

What exists:

- `apps/desktop/src-tauri/src/tools/dbmanager/` (M10):
  - `Core::new(dirs, config_path)`, `Core::reload_config()`;
  - the snapshot `DbmanagerSession` with `config: DbConfigView { path,
    status, hash }`, `configChanged`, `configFailClosed` and `diagnostics`;
  - `DbmanagerErrorKind::OpenerFailed`, which M10 already pins;
  - `lib/dbmanager.ts`, with names pinned in M10.
- `crates/fm-dbmanager/src/config/load.rs`: `LoadedConfig::load`, `hash`,
  `changed_on_disk`.
- `examples/dbmanager/starter.toml` (M3). It is commented and inert: it
  parses with zero diagnostics to the built-in editable defaults.
- `apps/desktop/src-tauri/src/tools/tarpack/watch.rs`: `ManifestWatcher`.
  It is a `notify-debouncer-mini` watcher on the file's **parent directory**
  (editors save by replacing the file). It has a 300 ms debounce and counts
  only `DebouncedEventKind::Any` for the file's name.
- How tarpack wires its events: `tools/tarpack/mod.rs`
  (`EVENT_MANIFEST_CHANGED = "tarpack://manifest-changed"`, `arm_watcher`,
  `app.emit`) and `lib/tarpack.ts` (`onManifestChanged`, built on `on` from
  `lib/tauri.ts`).
- How tarpack opens a file in the system editor: `tarpack_open_in_editor`,
  through `tauri_plugin_opener::OpenerExt::open_path`, with the path from
  state and never from the UI. That is Rust-side use, so it needs no webview
  capability.

Rules that bind this task:

- **Shared code goes in `fm-core`, and tools never import each other.** The
  watcher is needed by two tools, so move it into `fm-core`, and switch the
  Tar Packager to it without changing its behaviour. `fm-core` must stay free
  of tauri.
- Only `tools/mod.rs::register` touches the builder. Arm the watcher from a
  command that has an `AppHandle`, as tarpack does. Do not add `setup`.
- Tests never touch the real config location. They use temp-dir paths
  injected through `Core::new`.
- `capabilities/default.json` and the CSP do not change. The opener is used
  from Rust only, and `core:event:allow-listen` is already granted.
- fmt and clippy pedantic pass, with only a scoped `#[expect]`. Boundary
  types are generated, with no `bigint`.

### Shared watcher (`crates/fm-core/src/watch.rs`)

Move `ManifestWatcher` here as `FileWatcher` with the same behaviour and the
same debounce:

```rust
pub struct FileWatcher { /* … */ }
impl FileWatcher {
    /// Calls `on_change` once per burst of changes to the file at `path`
    /// (created, written, replaced, renamed into place, or deleted).
    pub fn start(path: &Path, on_change: impl Fn() + Send + 'static) -> Result<FileWatcher, String>;
    pub fn path(&self) -> &Path;
}
```

- **New behaviour: a missing directory.** If the file's parent directory
  does not exist (`%ProgramData%\FileManager` before anyone creates a
  config), watch the deepest existing ancestor non-recursively. When the next
  path component appears, move the watch down to it, until the parent
  directory is watched. Call `on_change` when the file itself appears. Never
  watch recursively, because `%ProgramData%` is large.
- `fm-core/Cargo.toml` gains `notify-debouncer-mini`, at the version the
  shell already uses. The shell keeps it only if it still needs it directly.
- `tools/tarpack/watch.rs` becomes a thin use of `fm_core::watch::FileWatcher`,
  or is deleted, with `tools/tarpack/mod.rs` using `FileWatcher` directly.
  Tarpack's existing watcher tests move with the code or keep passing
  unchanged.

### Automatic reload and the event

- `pub const EVENT_CONFIG_CHANGED: &str = "dbmanager://config-changed";`
- Arm the watcher once per app run, on the first `dbmanager_session` call,
  which has the `AppHandle`. Keep it in `DbmanagerState` next to `core`. Skip
  arming when the config path is `None`.
- The watcher callback runs on the debouncer's thread:
  1. lock `core`;
  2. `reload_config()`;
  3. unlock;
  4. if the new hash differs from the one before, `app.emit(EVENT_CONFIG_CHANGED,
     DbmanagerConfigChanged { hash, status })`.

  The emit happens **after** the reload, so a client that receives the event
  and calls `session()` sees the new config.
- Type, exported through `EXPORTERS`:

  ```rust
  #[serde(rename_all = "camelCase")]
  pub struct DbmanagerConfigChanged { pub hash: String, pub status: ConfigStatus }
  ```

- **A reload never touches the database, the drafts, or anything else.**
  `set_config` only re-resolves the model. Drafts live in the UI, which
  re-checks them. A write already running holds the `core` lock, so the
  reload waits for it. A write that starts after the reload with the old
  hash gets `ConfigChangedOnDisk` (M10).
- If the watcher cannot start (an unreadable `%ProgramData%`, say), log it
  the way tarpack does. The app still works: `configChanged` in `session()`
  and the write-path reload cover it.

### Developer-mode "Edit config" (`dbmanager_edit_config`)

```rust
pub struct EditConfigOutcome { pub created: bool, pub session: DbmanagerSession }
#[tauri::command]
pub async fn dbmanager_edit_config(app: AppHandle, state: State<'_, DbmanagerState>)
    -> Result<EditConfigOutcome, DbmanagerError>;
```

The path is taken from state. The command never takes a path argument.

1. If the config path is `None`, fail with `Io` ("the config location could
   not be determined").
2. If the file does not exist:
   1. create the parent directory (`create_dir_all`);
   2. write `include_str!("…/examples/dbmanager/starter.toml")` with
      `OpenOptions::create_new(true)`, so an existing file is never
      replaced. If another process created it in between, `AlreadyExists`
      is not an error: go on and open the file.
   3. set `created = true`.

   A permission error is `Io`, with a message naming the folder. On Windows,
   ordinary users may create files under `%ProgramData%`, but an existing
   file created by an administrator may not be writable; that does not
   matter, because the app only opens it.
3. `reload_config()`, then re-arm the watcher on the now-existing directory
   (as tarpack's `arm_watcher(…, force = true)` does).
4. Open the file in the system editor with `app.opener().open_path(path,
   None::<&str>)`. A path that is not valid Unicode, or an opener error, is
   `OpenerFailed`.
5. Return `EditConfigOutcome { created, session }`.

### Client additions (`apps/desktop/src/lib/dbmanager.ts`)

Add these names, which are pinned:

- `editConfig(): Promise<EditConfigOutcome>`, which calls
  `dbmanager_edit_config`. Its doc comment says it is for developer mode
  only, as gated by the view, and that it creates a commented starter file
  when none exists.
- `onConfigChanged(handler: (payload: DbmanagerConfigChanged) => void):
  Promise<UnlistenFn>`. It is built on `on` from `./tauri`, mirroring
  `onManifestChanged` in `lib/tarpack.ts`. Its doc comment says the backend
  has already reloaded: re-fetch `session()` and re-check drafts.
- `CONFIG_CHANGED_EVENT = "dbmanager://config-changed"`, exported as tarpack
  exports its event names.

Extend `dbmanager.test.ts` with a row for `editConfig`, and a test that
`onConfigChanged` listens to the right event name.

## Files

Create:

- `crates/fm-core/src/watch.rs`, with its tests.

Edit:

- `crates/fm-core/Cargo.toml`, `crates/fm-core/src/lib.rs`;
- `apps/desktop/src-tauri/Cargo.toml` (only if the direct
  `notify-debouncer-mini` dependency becomes unused);
- `apps/desktop/src-tauri/src/tools/tarpack/{watch.rs, mod.rs}` (switch to
  `FileWatcher`, behaviour unchanged);
- `apps/desktop/src-tauri/src/tools/dbmanager/{mod.rs, core.rs, types.rs,
  tests.rs}`;
- `apps/desktop/src-tauri/src/tools/mod.rs` (register
  `dbmanager_edit_config`);
- `apps/desktop/src-tauri/src/generated_types.rs`, and the generated files;
- `apps/desktop/src/lib/dbmanager.ts`, `apps/desktop/src/lib/dbmanager.test.ts`;
- `Cargo.lock`;
- `CLAUDE.md`: the layout lines for `fm-core` (`watch.rs`, the shared file
  watcher) and `tools/dbmanager` (the config watcher and `edit_config`); the
  tarpack `watch.rs` line, if that file is removed.

Do not touch the views, `App.tsx`, `devMode.tsx`, capabilities, the CSP, or
`docs/plans/`.

## Acceptance criteria

**Shared watcher** (`fm-core`, in temp dirs)

- Writing, replacing (write a temp file and rename it over), and deleting
  the watched file each give exactly one callback per burst.
- Changes to other files in the directory give none.
- A watched path whose parent directory does not exist yet fires once the
  directory and then the file are created.
- No recursive watch is used.

**Tar Packager**

- Behaviour is unchanged: all its existing tests pass, and its manifest
  watcher still reports external edits.

**Dbmanager reload** (driving `Core` and the watch callback directly, without
a Tauri runtime; factor the callback body into a plain function that returns
the payload to emit, or `None`)

- Editing the temp config:
  - reloads it;
  - updates `session().config.hash` and `tables` (for example a table
    switched to `read-only`);
  - keeps the open database (same `Session` instance, no reopen);
  - yields a `DbmanagerConfigChanged` payload.
- **The event follows the completed reload.** In
  `event_emitted_after_reload_completes`, the factored callback is given an
  emit closure that, when called, immediately calls `Core::session()` (as a
  UI handler would). Inside the closure, the snapshot already has the new
  `config.hash`, diagnostics and `tables`. The `core` lock must not be held
  while emitting, so the closure can take it without deadlocking. Assert
  both: the new state is visible, and nothing deadlocks.
- An edit that does not change the bytes yields no payload.
- Breaking the TOML makes the status `failClosed` with a reason. Deleting
  the file makes it `missing`.

**`dbmanager_edit_config` core logic** (in temp dirs, with the opener call
behind a seam, such as a function parameter, that tests replace)

- A missing file and a missing directory: both are created, the file's bytes
  equal `starter.toml`, `created` is true, the status becomes `loaded`, and
  the opener seam is called with the path.
- An existing file is never modified (same bytes and modification time), and
  `created` is false.
- An opener failure maps to `OpenerFailed`. A config path of `None` gives
  `Io`.

**Contract**

- The event name is pinned by a test. `DbmanagerConfigChanged` and
  `EditConfigOutcome` are generated.
- `builder_hooks_only_in_tool_registry` and `capabilities_are_minimal` pass,
  so capabilities are unchanged.

**Build**

- `npm run typecheck && npm run lint && npm run test`, fmt, clippy pedantic
  and `cargo test --workspace` pass.

## Tests proving completion

`fm-core`:

- `watch_reports_write_replace_delete`
- `watch_ignores_other_files`
- `watch_waits_for_missing_directory`

Shell:

- reload and event:
  - `config_edit_reloads_and_emits`
  - `event_emitted_after_reload_completes`
  - `unchanged_bytes_do_not_emit`
  - `broken_config_fails_closed_on_reload`
  - `deleted_config_is_missing`
- edit config:
  - `edit_config_creates_starter`
  - `edit_config_never_overwrites`
  - `edit_config_opener_failure`
  - `edit_config_without_location`
- `config_changed_event_name_pinned`

Tarpack: its existing watcher tests.

Frontend: the new `dbmanager.test.ts` rows.

Run `cargo test --workspace`, regenerate as in `CLAUDE.md`, then
`npm run typecheck && npm run lint && npm run test`.

## Out of scope

- The UI: banners, the developer-mode button, and re-checking drafts on the
  event. These belong to the ui-implementer.
- The window-close guard (M14).
- Writing to the config from the app in any other way.

## Risks

- Moving the watcher touches the Tar Packager. Keep its behaviour
  byte-for-byte the same, and let its tests prove it.
- File-system notification on network-redirected `%ProgramData%` setups can
  be unreliable. The write-path reload and `configChanged` are the fallback.
- A reload that runs during a long backup waits on the `core` lock. That is
  acceptable, because the config change is applied right after.
