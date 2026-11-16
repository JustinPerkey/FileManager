//! Opening a user's `SQLite` file safely.

use rusqlite::config::DbConfig;
use rusqlite::{Connection, ErrorCode, OpenFlags};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

const READ_ONLY_REASON: &str = "the file or its folder is read-only";

/// Why a database could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// Relative paths resolve against an invisible working directory.
    NotAbsolute(PathBuf),
    /// Missing, or a directory.
    NotAFile(PathBuf),
    /// `SQLite` needs a UTF-8 path.
    NotUnicode(PathBuf),
    /// `SQLite` says the file is not a database.
    NotADatabase(PathBuf, String),
    /// Anything else, with `SQLite`'s message.
    Io(PathBuf, String),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::NotAbsolute(p) => write!(f, "{} is not an absolute path.", p.display()),
            OpenError::NotAFile(p) => write!(f, "{} is not an existing file.", p.display()),
            OpenError::NotUnicode(p) => {
                write!(
                    f,
                    "{} cannot be opened: its path is not valid Unicode.",
                    p.display()
                )
            }
            OpenError::NotADatabase(p, m) => {
                write!(f, "{} is not a SQLite database: {m}", p.display())
            }
            OpenError::Io(p, m) => write!(f, "{} could not be opened: {m}", p.display()),
        }
    }
}

impl std::error::Error for OpenError {}

/// An open `SQLite` database file.
#[derive(Debug)]
pub struct Database {
    conn: Connection,
    path: PathBuf,
    read_only_reason: Option<String>,
}

fn is_retryable(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, _)
            if matches!(f.code, ErrorCode::ReadOnly | ErrorCode::CannotOpen)
    )
}

impl Database {
    /// Open an existing `SQLite` file, read-write when possible, else read-only.
    /// Never creates a file and never changes persistent settings of it.
    ///
    /// # Errors
    ///
    /// See [`OpenError`].
    pub fn open(path: &Path) -> Result<Database, OpenError> {
        let owned = path.to_path_buf();
        if path.to_str().is_none() {
            return Err(OpenError::NotUnicode(owned));
        }
        if !path.is_absolute() {
            return Err(OpenError::NotAbsolute(owned));
        }
        if !path.is_file() {
            return Err(OpenError::NotAFile(owned));
        }

        // No SQLITE_OPEN_CREATE and no SQLITE_OPEN_URI.
        let rw = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let (conn, mut reason) = match Connection::open_with_flags(path, rw) {
            Ok(c) => (c, None),
            Err(e) if is_retryable(&e) => {
                let ro = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
                let c = Connection::open_with_flags(path, ro).map_err(|e| classify(&owned, &e))?;
                (c, Some(READ_ONLY_REASON.to_owned()))
            }
            Err(e) => return Err(classify(&owned, &e)),
        };

        conn.query_row("SELECT count(*) FROM sqlite_schema", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::SqliteFailure(f, m) if f.code == ErrorCode::NotADatabase => {
                OpenError::NotADatabase(owned.clone(), m.unwrap_or_else(|| f.to_string()))
            }
            other => classify(&owned, &other),
        })?;

        if reason.is_none() && conn.is_readonly("main").unwrap_or(false) {
            reason = Some(READ_ONLY_REASON.to_owned());
        }

        let io = |e: rusqlite::Error| OpenError::Io(owned.clone(), e.to_string());
        conn.pragma_update(None, "foreign_keys", true).map_err(io)?;
        conn.busy_timeout(Duration::from_secs(3)).map_err(io)?;
        conn.pragma_update(None, "trusted_schema", false)
            .map_err(io)?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)
            .map_err(io)?;

        Ok(Database {
            conn,
            path: owned,
            read_only_reason: reason,
        })
    }

    /// The file's path, as given to [`Database::open`].
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `Some(reason)` when the file could only be opened read-only.
    #[must_use]
    pub fn read_only_reason(&self) -> Option<&str> {
        self.read_only_reason.as_deref()
    }

    /// The connection.
    #[must_use]
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// The connection, mutably (for transactions).
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// `PRAGMA schema_version`.
    ///
    /// # Errors
    ///
    /// Any `SQLite` error.
    pub fn schema_version(&self) -> rusqlite::Result<i64> {
        self.conn
            .query_row("PRAGMA schema_version", [], |r| r.get(0))
    }
}

fn classify(path: &Path, e: &rusqlite::Error) -> OpenError {
    if let rusqlite::Error::SqliteFailure(f, _) = e {
        if f.code == ErrorCode::NotADatabase {
            return OpenError::NotADatabase(path.to_path_buf(), e.to_string());
        }
    }
    OpenError::Io(path.to_path_buf(), e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pragma_str(db: &Database, name: &str) -> String {
        db.conn()
            .query_row(&format!("PRAGMA {name}"), [], |r| r.get::<_, String>(0))
            .unwrap()
    }

    fn pragma_int(db: &Database, name: &str) -> i64 {
        db.conn()
            .query_row(&format!("PRAGMA {name}"), [], |r| r.get(0))
            .unwrap()
    }

    fn make_db(dir: &Path, mode: &str) -> PathBuf {
        let p = dir.join("t.db");
        let c = Connection::open(&p).unwrap();
        c.query_row(&format!("PRAGMA journal_mode = {mode}"), [], |_| Ok(()))
            .unwrap();
        c.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY);")
            .unwrap();
        p
    }

    #[test]
    fn open_rejects_relative_path() {
        assert!(matches!(
            Database::open(Path::new("relative.db")),
            Err(OpenError::NotAbsolute(_))
        ));
    }

    #[test]
    fn open_missing_file_is_not_created() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("missing.db");
        assert!(matches!(Database::open(&p), Err(OpenError::NotAFile(_))));
        assert!(!p.exists());
    }

    #[test]
    fn open_directory_is_not_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(matches!(
            Database::open(tmp.path()),
            Err(OpenError::NotAFile(_))
        ));
    }

    #[test]
    fn open_non_database_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("fake.db");
        std::fs::write(
            &p,
            "this is definitely not a sqlite database, just plain text.",
        )
        .unwrap();
        let err = Database::open(&p).unwrap_err();
        assert!(matches!(err, OpenError::NotADatabase(..)), "{err:?}");
        assert!(err.to_string().contains("fake.db"));
    }

    #[test]
    fn open_sets_pragmas() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(&make_db(tmp.path(), "delete")).unwrap();
        assert_eq!(pragma_int(&db, "foreign_keys"), 1);
        assert_eq!(pragma_int(&db, "trusted_schema"), 0);
        assert_eq!(db.read_only_reason(), None);
    }

    #[test]
    fn open_preserves_journal_mode_wal() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(&make_db(tmp.path(), "wal")).unwrap();
        assert_eq!(pragma_str(&db, "journal_mode"), "wal");
    }

    #[test]
    fn open_preserves_journal_mode_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(&make_db(tmp.path(), "delete")).unwrap();
        assert_eq!(pragma_str(&db, "journal_mode"), "delete");
    }

    #[test]
    fn open_readonly_file_falls_back_to_read_only() {
        #[cfg(unix)]
        if permissions_do_not_bind() {
            eprintln!("skipping: running as root, permissions do not bind");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let p = make_db(tmp.path(), "delete");
        let mut perm = std::fs::metadata(&p).unwrap().permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&p, perm).unwrap();
        let db = Database::open(&p).unwrap();
        assert!(db.read_only_reason().is_some());
        // Restore so the temp dir can be removed on every platform.
        let mut perm = std::fs::metadata(&p).unwrap().permissions();
        #[expect(clippy::permissions_set_readonly_false, reason = "temp file cleanup")]
        perm.set_readonly(false);
        drop(db);
        std::fs::set_permissions(&p, perm).unwrap();
    }

    #[cfg(unix)]
    fn permissions_do_not_bind() -> bool {
        // Root ignores file modes: probe with a read-only temp file.
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("probe");
        std::fs::write(&p, "x").unwrap();
        let mut perm = std::fs::metadata(&p).unwrap().permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&p, perm).unwrap();
        std::fs::OpenOptions::new().write(true).open(&p).is_ok()
    }

    #[test]
    fn schema_version_changes_after_ddl() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(&make_db(tmp.path(), "delete")).unwrap();
        let before = db.schema_version().unwrap();
        db.conn().execute_batch("CREATE TABLE u (x)").unwrap();
        assert_ne!(db.schema_version().unwrap(), before);
    }
}
