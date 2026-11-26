//! Reading the one config file. Separate from opening a database: its status
//! is shown even when no database is open.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{parse, Config, ConfigDiagnostic, ConfigReport, DiagnosticSeverity};

/// Hash of a file that does not exist.
const MISSING_HASH: &str = "missing";
/// Hash of a file that exists but could not be read.
const UNREADABLE_HASH: &str = "unreadable";
/// Hash of the config whose location is unknown.
const UNAVAILABLE_HASH: &str = "unavailable";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum ConfigStatus {
    Loaded,
    Missing,
    FailClosed,
}

/// The config file as read at one moment.
#[derive(Clone, Debug)]
pub struct LoadedConfig {
    path: PathBuf,
    status: ConfigStatus,
    report: ConfigReport,
    hash: String,
    /// False for [`LoadedConfig::location_unknown`]: there is no file to
    /// watch.
    located: bool,
}

fn hex_digest(bytes: &[u8]) -> String {
    fm_core::to_hex(&Sha256::digest(bytes))
}

fn fail_closed(reason: String, message: String) -> ConfigReport {
    ConfigReport {
        config: Config::fail_closed(),
        diagnostics: vec![ConfigDiagnostic {
            severity: DiagnosticSeverity::Error,
            message,
            line: None,
            col: None,
            table: None,
            column: None,
        }],
        fail_closed: Some(reason),
    }
}

impl LoadedConfig {
    /// Reads and parses `path`. A missing file is `Missing` (editable
    /// built-in defaults); a file that cannot be read, or whose parse is
    /// fail-closed, is `FailClosed`; anything else is `Loaded`. The hash is
    /// the SHA-256 (hex) of the exact bytes.
    #[must_use]
    pub fn load(path: &Path) -> LoadedConfig {
        let make = |status, report, hash: &str| LoadedConfig {
            path: path.to_path_buf(),
            status,
            report,
            hash: hash.to_owned(),
            located: true,
        };
        let unreadable = |why: &str, hash: &str| {
            let reason = format!("the config file could not be read: {why}");
            make(
                ConfigStatus::FailClosed,
                fail_closed(reason.clone(), format!("{reason}; everything is read-only")),
                hash,
            )
        };
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return make(
                    ConfigStatus::Missing,
                    ConfigReport {
                        config: Config::default(),
                        diagnostics: Vec::new(),
                        fail_closed: None,
                    },
                    MISSING_HASH,
                );
            }
            Err(e) => return unreadable(&e.to_string(), UNREADABLE_HASH),
        };
        let hash = hex_digest(&bytes);
        let Ok(text) = String::from_utf8(bytes) else {
            return unreadable("it is not valid UTF-8", &hash);
        };
        let report = parse(&text);
        let status = if report.fail_closed.is_some() {
            ConfigStatus::FailClosed
        } else {
            ConfigStatus::Loaded
        };
        make(status, report, &hash)
    }

    /// The config of an app that could not work out where its config file
    /// lives: fail-closed, with nothing on disk to compare against.
    #[must_use]
    pub fn location_unknown() -> LoadedConfig {
        let reason = "the config location could not be determined".to_owned();
        LoadedConfig {
            path: PathBuf::new(),
            status: ConfigStatus::FailClosed,
            report: fail_closed(reason.clone(), format!("{reason}; everything is read-only")),
            hash: UNAVAILABLE_HASH.to_owned(),
            located: false,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn status(&self) -> ConfigStatus {
        self.status
    }

    /// SHA-256 (hex) of the bytes read, `"missing"` for an absent file.
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }

    #[must_use]
    pub fn report(&self) -> &ConfigReport {
        &self.report
    }

    #[must_use]
    pub fn fail_closed_reason(&self) -> Option<&str> {
        self.report.fail_closed.as_deref()
    }

    /// Config-only diagnostics (no schema needed).
    #[must_use]
    pub fn diagnostics(&self) -> &[ConfigDiagnostic] {
        &self.report.diagnostics
    }

    /// True if the file now differs from what was loaded. Appearing,
    /// disappearing and becoming unreadable all count.
    #[must_use]
    pub fn changed_on_disk(&self) -> bool {
        if !self.located {
            return false;
        }
        let now = match std::fs::read(&self.path) {
            Ok(bytes) => hex_digest(&bytes),
            Err(e) if e.kind() == ErrorKind::NotFound => MISSING_HASH.to_owned(),
            Err(_) => UNREADABLE_HASH.to_owned(),
        };
        now != self.hash
    }
}
