//! Fixture backends for the Tar Packager: tests and the screenshot harness.
//! They never touch the disk: manifests are the `examples/tarpack/` text,
//! parsed by `fm_tarpack::manifest::parse` (never read from a file), and
//! paths are fake Windows display strings such as `C:\Packages\gateway`.
//!
//! **Timing** (the harness's `Step::WaitIdle` is a short fixed settle, not
//! idle detection). Every method answers synchronously, so a scenario that
//! shows a fixture state needs no `Wait`. The one exception is a drop: its
//! work runs on a thread of its own (it returns at once unless held with
//! [`FixtureTarpack::hold_drops`]), so a scenario that drops files should
//! add a `Step::Wait` of about 200 ms before it looks at the result. The
//! build half answers with errors until part B adds its fixtures.

use std::any::Any;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use fm_tarpack::archive::BuildSummary;
use fm_tarpack::format::ArchiveFormat;
use fm_tarpack::manifest::{parse, Diagnostic, ManifestView, ParseReport, Severity};
use fm_tarpack::session::{
    ArchiveFormatOption, BuildBlockedReason, DroppedAssignment, SessionEntry, SessionManifest,
    TarpackError, TarpackErrorKind, TarpackSession,
};
use fm_tarpack::sources::{DropOutcome, EntryStatus, Unmatched, UnmatchedReason};

use super::backend::{BuildWork, DropWork, TarpackBackend};

/// The example manifest: three entries, no errors. The fixtures show
/// [`gateway_full`], which adds two entries.
pub const GATEWAY: &str = include_str!("../../../../../examples/tarpack/example.toml");

/// Where the fixture manifest "is".
pub const MANIFEST_PATH: &str = "C:\\Packages\\gateway\\manifest.toml";

/// The output of the states that have one.
pub const OUTPUT_PATH: &str = "C:\\Releases\\gateway.tar.zst";

const RECENT: [&str; 3] = [
    MANIFEST_PATH,
    "C:\\Packages\\sensor-agent\\manifest.toml",
    "D:\\work\\edge-router\\deploy.toml",
];

const GATEWAY_CA: &str =
    "[[file]]\nid = \"gateway-ca\"\nsource = \"ca.pem\"\ndir = \"/etc/gateway/tls\"\n";

const GATEWAY_UNIT: &str = "[[file]]\nid = \"gateway-unit\"\nsource = \"gateway.service\"\ndir = \"/etc/systemd/system\"\n";

/// `gateway-ca` as the `tarpack-errors` state has it: a relative `dir` and a
/// mode that is not octal, so the entry fails with two errors.
const GATEWAY_CA_BAD: &str = "[[file]]\nid = \"gateway-ca\"\nsource = \"ca.pem\"\ndir = \"etc/gateway/tls\"\nmode = \"0898\"\n";

/// The gateway manifest every `tarpack-*` screenshot shows: the example's
/// three entries, plus `gateway-ca` and `gateway-unit` (five in all, like the
/// mock gateway of `docs/manual/screenshots/mock.js`).
#[must_use]
pub fn gateway_full() -> String {
    format!("{GATEWAY}\n{GATEWAY_CA}\n{GATEWAY_UNIT}")
}

/// The `tarpack-errors` manifest: `gateway-ca` fails with two errors and the
/// manifest has the shared-source warning of the reference image. The
/// diagnostics are those of `mock.js`: [`parse_text`] sets their texts,
/// lines and columns exactly (the parser's own wording differs in the
/// warning, whose second file the screenshot does not list).
#[must_use]
pub fn gateway_with_errors() -> String {
    format!("{GATEWAY}\n{GATEWAY_CA_BAD}\n{GATEWAY_UNIT}")
}

/// A manifest whose top level is wrong (an unknown key before `version`): every entry is withheld.
#[must_use]
pub fn gateway_withheld() -> String {
    format!("default = 1\n{GATEWAY}")
}

/// Parses a fixture manifest. The `tarpack-errors` text gets the diagnostics
/// of the reference image ([`mock_errors`]).
fn parse_text(text: &str) -> ParseReport {
    let mut report = parse(text);
    if text == gateway_with_errors() {
        mock_errors(&mut report);
    }
    report
}

/// The diagnostics of the `tarpack-errors` reference image, from `mock.js`.
fn mock_errors(report: &mut ParseReport) {
    let error = |line, col, message: &str| Diagnostic {
        severity: Severity::Error,
        line,
        col,
        entry_id: Some("gateway-ca".to_string()),
        message: message.to_string(),
    };
    if let Some(failure) = report.failures.first_mut() {
        failure.index = 4;
        failure.line = 33;
        failure.errors = vec![
            error(
                35,
                7,
                "file `gateway-ca`: dir must be absolute (start with `/`)",
            ),
            error(
                36,
                8,
                "file `gateway-ca`: mode: mode `0898` is not an octal string",
            ),
        ];
    }
    report.warnings = vec![Diagnostic {
        severity: Severity::Warning,
        line: 41,
        col: 10,
        entry_id: Some("gateway-unit".to_string()),
        message: "file `gateway-unit`: source `gateway.service` matches file `gateway-svc` (compared case-insensitively); a dropped file cannot tell them apart, so pick their files per row".to_string(),
    }];
}

fn assigned_path(id: &str) -> String {
    format!("C:\\Builds\\gateway\\{id}")
}

/// Which screenshot state a fixture starts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TarpackFixture {
    /// `tarpack-empty`: nothing loaded, nothing recent.
    Empty,
    /// `tarpack-recent`: the manifest loaded and three recent manifests (the
    /// scenario opens the menu).
    Recent,
    /// `tarpack-loaded` (and `-format-open`): a manifest, nothing assigned.
    Loaded,
    /// `tarpack-errors`: a failed entry and a warning; the passed entries are
    /// assigned and an output is set.
    Errors,
    /// `tarpack-missing`: everything assigned, one source file missing.
    Missing,
    /// `tarpack-partial`: some entries assigned, an output set.
    Partial,
    /// `tarpack-changed`: [`TarpackFixture::Ready`]; the scenario injects the
    /// file-changed signal (`Step::Watcher`).
    Changed,
    /// `tarpack-drag`: [`TarpackFixture::Loaded`]; the scenario drags files
    /// over the window.
    Drag,
    /// `tarpack-dropped`: [`TarpackFixture::Loaded`]; the scenario drops
    /// `gateway-bin`, `gateway-start` and `gateway.conf` (matched) and a
    /// `README.md` (not in the manifest). The fixture matches by file name
    /// like the real thing, without the disk. Add a `Wait` of about 200 ms.
    Dropped,
    /// `tarpack-withheld`: a top-level error withholds every entry.
    Withheld,
    /// `tarpack-ready`: everything assigned, an output set, can build.
    Ready,
}

/// A closed gate holds a drop's work until opened, so a test can look at the
/// model while it matches.
#[derive(Clone, Default)]
pub struct Gate(Arc<(Mutex<bool>, Condvar)>);

impl Gate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Lets every job waiting on this gate go on.
    pub fn open(&self) {
        let (open, changed) = &*self.0;
        *open.lock().unwrap_or_else(PoisonError::into_inner) = true;
        changed.notify_all();
    }

    fn wait(&self, at_most: Duration) {
        let (open, changed) = &*self.0;
        let guard = open.lock().unwrap_or_else(PoisonError::into_inner);
        let _ = changed
            .wait_timeout_while(guard, at_most, |open| !*open)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

/// A backend call that can be made to panic ([`FixtureTarpack::panic_on`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Open,
    OpenRecent,
    Reload,
    Create,
    Assign,
    Clear,
    BeginDrop,
    FinishDrop,
}

/// One call the model made on the backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Session,
    RecentManifests,
    Open(PathBuf),
    OpenRecent(usize),
    Reload,
    Create(PathBuf),
    Assign(String, PathBuf),
    Clear(String),
    BeginDrop,
    FinishDrop,
}

struct State {
    /// The text the next open, create or reload parses.
    text: String,
    report: Option<ParseReport>,
    path: Option<String>,
    /// Entry id to its assigned display path and status.
    assigned: BTreeMap<String, (String, EntryStatus)>,
    output: Option<String>,
    format: ArchiveFormat,
    warning: Option<String>,
    recent: Vec<String>,
    calls: Vec<Call>,
    fail_next: Option<TarpackError>,
    stale_drops: usize,
    panic_sessions: usize,
    panic_commands: Vec<Command>,
    drop_gate: Option<Gate>,
    drop_outcome: Option<DropOutcome>,
}

/// A scripted [`TarpackBackend`] that mirrors the real session's rules
/// (assignments by id, a drop matched by file name, a drop applied only when
/// the session did not change) without the disk. Clones share one state, so a
/// test keeps a handle after the model owns the backend.
#[derive(Clone)]
pub struct FixtureTarpack {
    state: Arc<Mutex<State>>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

fn snapshot(s: &State) -> TarpackSession {
    let formats = ArchiveFormatOption::all();
    let state_warning = s.warning.clone();
    let (Some(report), Some(path)) = (&s.report, &s.path) else {
        return TarpackSession {
            manifest: None,
            output_path: None,
            format: ArchiveFormat::Tar,
            formats,
            suggested_output_name: None,
            ready_count: 0,
            total_count: 0,
            can_build: false,
            build_blocked_reason: Some(BuildBlockedReason::NoManifest),
            state_warning,
        };
    };
    let view = ManifestView::from(&report.manifest);
    let stem = Path::new(path.rsplit('\\').next().unwrap_or(path))
        .file_stem()
        .map(OsStr::to_os_string)
        .unwrap_or_default();
    let name = if view.name.is_empty() {
        stem.to_string_lossy().into_owned()
    } else {
        view.name
    };
    let base = view
        .output_name
        .as_deref()
        .map_or_else(|| stem.clone(), std::ffi::OsString::from);
    let suggested = s.format.with_extension(&base);
    let entries: Vec<SessionEntry> = view
        .entries
        .into_iter()
        .map(|v| {
            let (assigned, status) = s.assigned.get(&v.id).map_or_else(
                || (None, EntryStatus::Unassigned),
                |(p, st)| (Some(p.clone()), *st),
            );
            SessionEntry {
                view: v,
                assigned,
                status,
            }
        })
        .collect();
    let total = u32::try_from(entries.len()).unwrap_or(u32::MAX);
    let ready = u32::try_from(
        entries
            .iter()
            .filter(|e| e.status == EntryStatus::Ready)
            .count(),
    )
    .unwrap_or(u32::MAX);
    let reason = if entries.is_empty() {
        Some(BuildBlockedReason::NoEntries)
    } else if entries.iter().any(|e| e.status == EntryStatus::Missing) {
        Some(BuildBlockedReason::EntriesNotReady)
    } else if ready == 0 {
        Some(BuildBlockedReason::NothingLoaded)
    } else if s.output.is_none() {
        Some(BuildBlockedReason::NoOutput)
    } else {
        None
    };
    TarpackSession {
        manifest: Some(SessionManifest {
            path: path.clone(),
            name,
            output_name: view.output_name,
            hash: "9f2c".to_string(),
            entries,
            entries_withheld: report.entries_withheld,
            errors: report.errors.clone(),
            failed_entries: report.failures.clone(),
            error_count: report.error_count(),
            warnings: report.warnings.clone(),
        }),
        output_path: s.output.clone(),
        format: s.format,
        formats,
        suggested_output_name: Some(suggested.to_string_lossy().into_owned()),
        ready_count: ready,
        total_count: total,
        can_build: reason.is_none(),
        build_blocked_reason: reason,
        state_warning,
    }
}

fn display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

impl State {
    fn empty() -> Self {
        State {
            text: gateway_full(),
            report: None,
            path: None,
            assigned: BTreeMap::new(),
            output: None,
            format: ArchiveFormat::TarZst,
            warning: None,
            recent: Vec::new(),
            calls: Vec::new(),
            fail_next: None,
            stale_drops: 0,
            panic_sessions: 0,
            panic_commands: Vec::new(),
            drop_gate: None,
            drop_outcome: None,
        }
    }

    fn failure(&mut self) -> Result<(), TarpackError> {
        self.fail_next.take().map_or(Ok(()), Err)
    }

    fn load(&mut self, path: String) {
        self.report = Some(parse_text(&self.text));
        self.assigned.clear();
        self.recent.retain(|r| *r != path);
        self.recent.insert(0, path.clone());
        self.path = Some(path);
    }

    fn ids(&self) -> Vec<String> {
        self.report.as_ref().map_or_else(Vec::new, |r| {
            ManifestView::from(&r.manifest)
                .entries
                .into_iter()
                .map(|e| e.id)
                .collect()
        })
    }

    fn assign_ready(&mut self, ids: &[&str]) {
        for id in ids {
            self.assigned
                .insert((*id).to_string(), (assigned_path(id), EntryStatus::Ready));
        }
    }

    fn loaded_entry(&self, id: &str) -> Result<(), TarpackError> {
        if self.report.is_none() {
            return Err(TarpackError::new(
                TarpackErrorKind::NoManifest,
                "no manifest is loaded",
            ));
        }
        if self.ids().iter().any(|i| i == id) {
            Ok(())
        } else {
            Err(TarpackError::for_entry(
                TarpackErrorKind::UnknownEntry,
                format!("the manifest has no entry `{id}` that can be built"),
                id,
            ))
        }
    }
}

impl FixtureTarpack {
    fn from_state(state: State) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    /// Nothing loaded, nothing recent.
    #[must_use]
    pub fn empty() -> Self {
        Self::from_state(State::empty())
    }

    /// The example manifest loaded with nothing assigned, and the recent
    /// list.
    #[must_use]
    pub fn loaded() -> Self {
        let mut s = State::empty();
        s.recent = RECENT.iter().map(|r| (*r).to_string()).collect();
        s.load(MANIFEST_PATH.to_string());
        Self::from_state(s)
    }

    /// The state a screenshot scenario starts in.
    #[must_use]
    pub fn from_fixture(fixture: TarpackFixture) -> Self {
        use TarpackFixture as F;
        let this = match fixture {
            F::Empty => return Self::empty(),
            F::Recent | F::Loaded | F::Drag | F::Dropped => return Self::loaded(),
            F::Errors => Self::loaded().with_text(&gateway_with_errors()),
            F::Withheld => Self::loaded().with_text(&gateway_withheld()),
            F::Missing | F::Partial | F::Changed | F::Ready => Self::loaded(),
        };
        let mut s = lock(&this.state);
        s.load(MANIFEST_PATH.to_string());
        let ids = s.ids();
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        match fixture {
            F::Partial => s.assign_ready(&["gateway-bin", "gateway-conf", "gateway-start"]),
            F::Missing => {
                s.assign_ready(&ids);
                s.assigned.insert(
                    "gateway-conf".to_string(),
                    (assigned_path("gateway-conf"), EntryStatus::Missing),
                );
            }
            F::Ready | F::Changed | F::Errors => s.assign_ready(&ids),
            _ => {}
        }
        if matches!(
            fixture,
            F::Ready | F::Changed | F::Errors | F::Missing | F::Partial
        ) {
            s.output = Some(OUTPUT_PATH.to_string());
        }
        drop(s);
        this
    }

    /// The text the next open, create or reload parses (and, with
    /// [`FixtureTarpack::from_fixture`], what is loaded now).
    #[must_use]
    pub fn with_text(self, text: &str) -> Self {
        lock(&self.state).text = text.to_string();
        self
    }

    /// Changes what the next open, create or reload parses; the loaded
    /// manifest changes when something reloads it.
    pub fn set_text(&self, text: &str) {
        lock(&self.state).text = text.to_string();
    }

    /// A state warning in every session.
    #[must_use]
    pub fn with_state_warning(self, warning: &str) -> Self {
        lock(&self.state).warning = Some(warning.to_string());
        self
    }

    /// Changes the state warning of later sessions.
    pub fn set_state_warning(&self, warning: Option<&str>) {
        lock(&self.state).warning = warning.map(str::to_string);
    }

    /// The next open, reload, create, assign, clear or drop step fails with
    /// `error`.
    pub fn fail_next(&self, error: TarpackError) {
        lock(&self.state).fail_next = Some(error);
    }

    /// The next `n` drops find the session changed when they finish.
    pub fn stale_drops(&self, n: usize) {
        lock(&self.state).stale_drops = n;
    }

    /// The next `n` calls of `session` panic (the Worker reports it).
    pub fn panic_on_session(&self, n: usize) {
        lock(&self.state).panic_sessions = n;
    }

    /// The next call of `command` panics, once, before it changes anything
    /// (the Worker reports it). A panic in `FinishDrop` leaves the drop's
    /// files unassigned.
    pub fn panic_on(&self, command: Command) {
        lock(&self.state).panic_commands.push(command);
    }

    /// Replaces the recent list the backend holds, without telling the
    /// model (which keeps the copy it fetched): tests use it to make the
    /// model's list out of date.
    pub fn set_recent(&self, recent: &[&str]) {
        lock(&self.state).recent = recent.iter().map(|r| (*r).to_string()).collect();
    }

    /// Holds every drop's work until `gate` opens (at most 20 s).
    pub fn hold_drops(&self, gate: &Gate) {
        lock(&self.state).drop_gate = Some(gate.clone());
    }

    /// Every drop yields exactly `outcome`, whatever the paths.
    pub fn script_drop(&self, outcome: DropOutcome) {
        lock(&self.state).drop_outcome = Some(outcome);
    }

    /// Every call so far, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<Call> {
        lock(&self.state).calls.clone()
    }

    fn record(s: &mut State, call: Call) {
        s.calls.push(call);
    }

    /// Records `call` and panics when `command` was set to
    /// ([`FixtureTarpack::panic_on`]). The lock is released first.
    fn enter(&self, call: Call, command: Command) -> MutexGuard<'_, State> {
        let mut s = lock(&self.state);
        Self::record(&mut s, call);
        if let Some(at) = s.panic_commands.iter().position(|c| *c == command) {
            s.panic_commands.remove(at);
            drop(s);
            panic!("{command:?} exploded");
        }
        s
    }
}

struct FixtureDropWork {
    /// Per entry: id, lowercase source, assigned display path.
    entries: Vec<(String, String, Option<String>)>,
    scripted: Option<DropOutcome>,
    gate: Option<Gate>,
    outcome: Option<DropOutcome>,
}

impl DropWork for FixtureDropWork {
    fn run(&mut self, paths: &[PathBuf]) {
        if let Some(gate) = &self.gate {
            gate.wait(Duration::from_secs(20));
        }
        if let Some(outcome) = &self.scripted {
            self.outcome = Some(outcome.clone());
            return;
        }
        let mut outcome = DropOutcome::default();
        for p in paths {
            let name = p
                .to_string_lossy()
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or_default()
                .to_lowercase();
            match self.entries.iter().find(|(_, source, _)| *source == name) {
                None => outcome.unmatched.push(Unmatched {
                    path: p.clone(),
                    reason: UnmatchedReason::NoEntry,
                }),
                Some((_, _, Some(assigned))) if *assigned == display(p) => {
                    outcome.unmatched.push(Unmatched {
                        path: p.clone(),
                        reason: UnmatchedReason::AlreadyAssigned,
                    });
                }
                Some((id, _, _)) => outcome.matched.push((id.clone(), p.clone())),
            }
        }
        self.outcome = Some(outcome);
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

fn no_build() -> TarpackError {
    TarpackError::new(
        TarpackErrorKind::NoOutput,
        "the fixture cannot build (the build fixtures come with part B)",
    )
}

impl TarpackBackend for FixtureTarpack {
    fn session(&mut self) -> TarpackSession {
        let mut s = lock(&self.state);
        Self::record(&mut s, Call::Session);
        if s.panic_sessions > 0 {
            s.panic_sessions -= 1;
            drop(s);
            panic!("session exploded");
        }
        snapshot(&s)
    }

    fn recent_manifests(&mut self) -> Vec<String> {
        let mut s = lock(&self.state);
        Self::record(&mut s, Call::RecentManifests);
        s.recent.clone()
    }

    fn open(&mut self, path: &Path) -> Result<TarpackSession, TarpackError> {
        let mut s = self.enter(Call::Open(path.to_path_buf()), Command::Open);
        s.failure()?;
        s.load(display(path));
        Ok(snapshot(&s))
    }

    fn open_recent(
        &mut self,
        index: usize,
        expected_display: &str,
    ) -> Result<Option<TarpackSession>, TarpackError> {
        let mut s = self.enter(Call::OpenRecent(index), Command::OpenRecent);
        s.failure()?;
        let path = s.recent.get(index).cloned().ok_or_else(|| {
            TarpackError::new(
                TarpackErrorKind::ManifestUnreadable,
                "that manifest is no longer in the recent list",
            )
        })?;
        if path != expected_display {
            return Ok(None);
        }
        s.load(path);
        Ok(Some(snapshot(&s)))
    }

    fn reload(&mut self) -> Result<TarpackSession, TarpackError> {
        let mut s = self.enter(Call::Reload, Command::Reload);
        s.failure()?;
        if s.report.is_none() {
            return Err(TarpackError::new(
                TarpackErrorKind::NoManifest,
                "no manifest is loaded",
            ));
        }
        s.report = Some(parse_text(&s.text));
        let ids = s.ids();
        s.assigned.retain(|id, _| ids.contains(id));
        Ok(snapshot(&s))
    }

    fn create_from_example(&mut self, path: &Path) -> Result<TarpackSession, TarpackError> {
        let mut s = self.enter(Call::Create(path.to_path_buf()), Command::Create);
        s.failure()?;
        s.load(display(path));
        Ok(snapshot(&s))
    }

    fn assigned_path(&self, id: &str) -> Option<PathBuf> {
        lock(&self.state)
            .assigned
            .get(id)
            .map(|(path, _)| PathBuf::from(path))
    }

    fn assign(&mut self, id: &str, path: &Path) -> Result<TarpackSession, TarpackError> {
        let mut s = self.enter(
            Call::Assign(id.to_string(), path.to_path_buf()),
            Command::Assign,
        );
        s.failure()?;
        s.loaded_entry(id)?;
        s.assigned
            .insert(id.to_string(), (display(path), EntryStatus::Ready));
        Ok(snapshot(&s))
    }

    fn clear(&mut self, id: &str) -> Result<TarpackSession, TarpackError> {
        let mut s = self.enter(Call::Clear(id.to_string()), Command::Clear);
        s.failure()?;
        s.loaded_entry(id)?;
        s.assigned.remove(id);
        Ok(snapshot(&s))
    }

    fn begin_drop(&mut self) -> Result<Box<dyn DropWork>, TarpackError> {
        let mut s = self.enter(Call::BeginDrop, Command::BeginDrop);
        s.failure()?;
        if s.report.is_none() {
            return Err(TarpackError::new(
                TarpackErrorKind::NoManifest,
                "no manifest is loaded",
            ));
        }
        let entries = snapshot(&s)
            .manifest
            .map(|m| m.entries)
            .unwrap_or_default()
            .into_iter()
            .map(|e| (e.view.id, e.view.source.to_lowercase(), e.assigned))
            .collect();
        Ok(Box::new(FixtureDropWork {
            entries,
            scripted: s.drop_outcome.clone(),
            gate: s.drop_gate.clone(),
            outcome: None,
        }))
    }

    fn finish_drop(
        &mut self,
        work: Box<dyn DropWork>,
    ) -> Result<Option<DroppedAssignment>, TarpackError> {
        let mut s = self.enter(Call::FinishDrop, Command::FinishDrop);
        s.failure()?;
        let outcome = work
            .into_any()
            .downcast::<FixtureDropWork>()
            .ok()
            .and_then(|w| w.outcome)
            .ok_or_else(|| {
                TarpackError::new(TarpackErrorKind::Io, "the dropped files were never matched")
            })?;
        if s.stale_drops > 0 {
            s.stale_drops -= 1;
            return Ok(None);
        }
        for (id, path) in &outcome.matched {
            s.assigned
                .insert(id.clone(), (display(path), EntryStatus::Ready));
        }
        Ok(Some(DroppedAssignment {
            session: snapshot(&s),
            outcome,
        }))
    }

    fn set_output(&mut self, path: &Path) -> Result<TarpackSession, TarpackError> {
        let mut s = lock(&self.state);
        s.output = Some(display(path));
        Ok(snapshot(&s))
    }

    fn set_format(&mut self, format: ArchiveFormat) -> Result<TarpackSession, TarpackError> {
        let mut s = lock(&self.state);
        s.format = format;
        Ok(snapshot(&s))
    }

    fn begin_build(&mut self) -> Result<Box<dyn BuildWork>, TarpackError> {
        Err(no_build())
    }

    fn finish_build(&mut self, _work: Box<dyn BuildWork>) -> Result<BuildSummary, TarpackError> {
        Err(no_build())
    }

    fn abort_build(&mut self) {}

    fn manifest_path(&self) -> Option<PathBuf> {
        // A fixture never watches a file.
        None
    }

    fn manifest_path_for_open(&self) -> Result<PathBuf, TarpackError> {
        lock(&self.state)
            .path
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| TarpackError::new(TarpackErrorKind::NoManifest, "no manifest is loaded"))
    }

    fn reveal_target(&self) -> Result<PathBuf, TarpackError> {
        Err(TarpackError::new(
            TarpackErrorKind::NoOutput,
            "no archive has been built in this session",
        ))
    }
}
