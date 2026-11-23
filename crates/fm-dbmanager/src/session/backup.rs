//! The one-time backup taken before a session's first write: SQLite's online
//! backup API into a temp file beside the database, moved into place without
//! replacing anything.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, ErrorCode};

use super::{Session, SessionError};
use crate::db::BUSY_TIMEOUT;

/// Pages copied per `step`.
const STEP_PAGES: std::ffi::c_int = 256;
/// Pause between steps while the source is busy or locked.
const RETRY_PAUSE: std::time::Duration = std::time::Duration::from_millis(10);

/// How many names (`.bak`, `-1.bak`, ...) are tried before giving up.
pub(super) const MAX_ATTEMPTS: u32 = 100;

/// `yyyymmddThhmmssZ` for `secs` since the Unix epoch.
fn utc_stamp(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    // Civil date from a day count (Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn backup_name(file_name: &std::ffi::OsStr, stamp: &str, attempt: u32) -> OsString {
    let mut name = file_name.to_os_string();
    name.push(".");
    name.push(stamp);
    if attempt > 0 {
        name.push(format!("-{attempt}"));
    }
    name.push(".bak");
    name
}

fn failed(detail: impl Into<String>) -> SessionError {
    SessionError::BackupFailed {
        detail: detail.into(),
    }
}

/// How many times the copy may restart because another connection changed
/// the source mid-copy before it gives up with `Busy`.
///
/// Chosen over leaving it to the `data_version` race guard: the guard only
/// runs after the copy finishes, so a source that never stops changing would
/// keep the copy running. A restart shows as `remaining` not shrinking
/// between steps.
const MAX_RESTARTS: u32 = 5;

/// Runs the copy one `step` at a time, so a lock another program holds ends
/// in `Busy` after the session's busy timeout instead of spinning. The
/// timeout measures time without progress: every `More` step restarts it.
fn copy_pages(
    conn: &Connection,
    dest: &mut Connection,
    before_step: &mut dyn FnMut(),
) -> Result<(), SessionError> {
    let backup = Backup::new(conn, dest).map_err(|e| failed(e.to_string()))?;
    let mut start = Instant::now();
    let mut restarts = 0;
    let mut remaining = None;
    loop {
        before_step();
        match backup.step(STEP_PAGES) {
            Ok(StepResult::Done) => return Ok(()),
            Ok(StepResult::More) => {
                start = Instant::now();
                let now = backup.progress().remaining;
                if remaining.is_some_and(|before| now >= before) {
                    restarts += 1;
                    if restarts > MAX_RESTARTS {
                        return Err(SessionError::Busy);
                    }
                }
                remaining = Some(now);
            }
            Ok(StepResult::Busy | StepResult::Locked) => {
                if start.elapsed() >= BUSY_TIMEOUT {
                    return Err(SessionError::Busy);
                }
                std::thread::sleep(RETRY_PAUSE);
            }
            Ok(_) => return Err(failed("the backup stopped early")),
            Err(rusqlite::Error::SqliteFailure(f, _))
                if matches!(f.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) =>
            {
                return Err(SessionError::Busy);
            }
            Err(e) => return Err(failed(e.to_string())),
        }
    }
}

/// Copies `conn`'s main database beside `database` and returns the new
/// file's path. Never replaces an existing file; removes its temp file on
/// every failure.
pub(super) fn take_backup(
    conn: &Connection,
    database: &Path,
    stamp: &str,
) -> Result<PathBuf, SessionError> {
    let dir = database
        .parent()
        .ok_or_else(|| failed("the database has no folder"))?;
    let file_name = database
        .file_name()
        .ok_or_else(|| failed("the database has no file name"))?;
    let mut temp = tempfile::Builder::new()
        .prefix(".fm-backup-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| failed(e.to_string()))?;
    {
        let mut dest = Connection::open(temp.path()).map_err(|e| failed(e.to_string()))?;
        copy_pages(conn, &mut dest, &mut || {})?;
    }
    for attempt in 0..MAX_ATTEMPTS {
        let target = dir.join(backup_name(file_name, stamp, attempt));
        match temp.persist_noclobber(&target) {
            Ok(_) => return Ok(target),
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => temp = e.file,
            Err(e) => return Err(failed(e.error.to_string())),
        }
    }
    Err(failed("every backup name beside the database is taken"))
}

/// Takes the backup through `conn` if `backup` is still `None`, recording its
/// path.
///
/// It runs before the write transaction begins: SQLite refuses an online
/// backup (`Busy`) from the connection that holds `BEGIN IMMEDIATE`. So
/// another program could commit between the copy and the lock. `save` guards
/// that: it reads `PRAGMA data_version` before the copy and again after
/// `BEGIN IMMEDIATE`, and when it changed it rolls back, deletes the
/// never-announced copy and retakes it (up to 3 times, then `Busy`).
///
/// A backup taken before a save that then fails (`SchemaChanged`, `Conflict`,
/// `UnknownObject`, a constraint) is kept: it is a valid pre-write copy, and
/// `backup_created` reports it with the next successful write.
pub(crate) fn ensure_backup(
    backup: &mut Option<PathBuf>,
    stamp: Option<&str>,
    conn: &Connection,
    database: &Path,
) -> Result<(), SessionError> {
    if backup.is_some() {
        return Ok(());
    }
    let stamp = stamp.map_or_else(
        || {
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            utc_stamp(secs)
        },
        str::to_owned,
    );
    *backup = Some(take_backup(conn, database, &stamp)?);
    Ok(())
}

impl Session {
    /// The backup this session took (or adopted), if any.
    #[must_use]
    pub fn backup_path(&self) -> Option<&Path> {
        self.backup.as_deref()
    }

    /// Marks `backup` as the session's backup, already announced, without
    /// writing anything, so a reload of the same database does not take
    /// another. Use it only for the same database path.
    pub fn adopt_backup(&mut self, backup: PathBuf) {
        self.backup = Some(backup);
        self.backup_announced = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_is_utc_civil_time() {
        assert_eq!(utc_stamp(0), "19700101T000000Z");
        assert_eq!(utc_stamp(1_773_396_900), "20260313T101500Z");
        assert_eq!(utc_stamp(951_782_400 + 86_399), "20000229T235959Z");
    }

    #[test]
    fn names_are_built_from_os_strings() {
        let name = backup_name(std::ffi::OsStr::new("shop.db"), "20261113T101500Z", 0);
        assert_eq!(name, "shop.db.20261113T101500Z.bak");
        let name = backup_name(std::ffi::OsStr::new("shop.db"), "20261113T101500Z", 2);
        assert_eq!(name, "shop.db.20261113T101500Z-2.bak");
    }

    #[test]
    fn locked_source_gives_up_with_busy_and_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("t.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE t (a); INSERT INTO t VALUES (1);")
            .unwrap();
        let other = Connection::open(&db).unwrap();
        other.execute_batch("BEGIN EXCLUSIVE;").unwrap();
        let start = Instant::now();
        let err = take_backup(&conn, &db, "20261113T101500Z").unwrap_err();
        assert!(matches!(err, SessionError::Busy), "{err:?}");
        assert!(start.elapsed() < std::time::Duration::from_secs(15));
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert!(names.iter().all(|n| !n.to_string_lossy().contains("bak")));
        assert!(names
            .iter()
            .all(|n| !n.to_string_lossy().contains("fm-backup")));
    }

    #[test]
    fn restarting_copies_give_up_with_busy() {
        // A source another connection changes before every step restarts the
        // copy each time; the bound ends it with Busy.
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("t.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE t (a); INSERT INTO t VALUES (zeroblob(4000000));")
            .unwrap();
        let writer = Connection::open(&db).unwrap();
        let mut dest = Connection::open(dir.path().join("d.db")).unwrap();
        let mut steps = 0;
        let err = copy_pages(&conn, &mut dest, &mut || {
            steps += 1;
            writer.execute("INSERT INTO t VALUES (1)", []).unwrap();
        })
        .unwrap_err();
        assert!(matches!(err, SessionError::Busy), "{err:?}");
        assert!(steps <= 20, "gave up after {steps} steps");
    }
}
