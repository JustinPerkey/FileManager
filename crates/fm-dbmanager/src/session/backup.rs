//! The one-time backup taken before a session's first write: SQLite's online
//! backup API into a temp file beside the database, moved into place without
//! replacing anything.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::backup::Backup;
use rusqlite::Connection;

use super::{Session, SessionError};

/// How many names (`.bak`, `-1.bak`, ...) are tried before giving up.
const MAX_ATTEMPTS: u32 = 100;

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

/// Copies `conn`'s main database beside `database` and returns the new
/// file's path. Never replaces an existing file; removes its temp file on
/// failure.
pub(super) fn take_backup(
    conn: &Connection,
    database: &Path,
    stamp: &str,
) -> Result<PathBuf, String> {
    let dir = database
        .parent()
        .ok_or_else(|| "the database has no folder".to_owned())?;
    let file_name = database
        .file_name()
        .ok_or_else(|| "the database has no file name".to_owned())?;
    let temp = tempfile::Builder::new()
        .prefix(".fm-backup-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(|e| e.to_string())?;
    {
        let mut dest = Connection::open(temp.path()).map_err(|e| e.to_string())?;
        let backup = Backup::new(conn, &mut dest).map_err(|e| e.to_string())?;
        backup
            .run_to_completion(256, Duration::ZERO, None)
            .map_err(|e| e.to_string())?;
    }
    let mut temp = temp;
    for attempt in 0..MAX_ATTEMPTS {
        let target = dir.join(backup_name(file_name, stamp, attempt));
        match temp.persist_noclobber(&target) {
            Ok(_) => return Ok(target),
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => temp = e.file,
            Err(e) => return Err(e.error.to_string()),
        }
    }
    Err("every backup name beside the database is taken".to_owned())
}

impl Session {
    /// The backup this session took (or adopted), if any.
    #[must_use]
    pub fn backup_path(&self) -> Option<&Path> {
        self.backup.as_deref()
    }

    /// Marks `backup` as the session's backup without writing anything, so a
    /// reload of the same database does not take another. Use it only for
    /// the same database path.
    pub fn adopt_backup(&mut self, backup: PathBuf) {
        self.backup = Some(backup);
    }

    /// Takes the backup if none exists yet. `Some(path)` only when it made one.
    pub(crate) fn ensure_backup(&mut self) -> Result<Option<PathBuf>, SessionError> {
        if self.backup.is_some() {
            return Ok(None);
        }
        let now = || {
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            utc_stamp(secs)
        };
        #[cfg(test)]
        let stamp = self.backup_stamp.clone().unwrap_or_else(now);
        #[cfg(not(test))]
        let stamp = now();
        let path = take_backup(self.database.conn(), self.database.path(), &stamp)
            .map_err(SessionError::BackupFailed)?;
        self.backup = Some(path.clone());
        Ok(Some(path))
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
}
