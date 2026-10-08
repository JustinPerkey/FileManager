//! The Schedule Creator's session logic as plain functions, with no Tauri
//! types. The parse and merge steps go through [`Hooks`], which default to
//! `fm_schedule::parse` and `fm_schedule::merge`; tests swap in fakes.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use fm_schedule::{MergeError, MergeOutcome, ParseError, Schedule};

use super::types::{ApplySummary, ScheduleError, ScheduleErrorKind as K, ScheduleSession};

pub(super) type ParseFn = fn(&str) -> Result<Schedule, ParseError>;
pub(super) type MergeFn = fn(&Schedule, &str) -> Result<MergeOutcome, MergeError>;

/// The two domain steps the session calls.
#[derive(Clone, Copy)]
pub(super) struct Hooks {
    pub parse: ParseFn,
    pub merge: MergeFn,
}

impl Default for Hooks {
    fn default() -> Self {
        Hooks {
            parse: fm_schedule::parse,
            merge: fm_schedule::merge,
        }
    }
}

/// The text file, and what parsing it produced.
struct Text {
    path: PathBuf,
    parsed: Result<Schedule, ScheduleError>,
}

#[derive(Default)]
pub(super) struct Core {
    hooks: Hooks,
    text: Option<Text>,
    xml: Option<PathBuf>,
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Reads `path` as UTF-8 text, dropping a leading byte-order mark.
fn read_text(path: &Path, kind: K) -> Result<String, ScheduleError> {
    let bytes = fs::read(path)
        .map_err(|e| ScheduleError::new(kind, format!("reading {}: {e}", path.display())))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| ScheduleError::new(kind, format!("{} is not UTF-8 text", path.display())))?;
    Ok(match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_owned(),
        None => text,
    })
}

fn require_file(path: &Path) -> Result<(), ScheduleError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(ScheduleError::new(
            K::NotAFile,
            format!("{} is not a file", path.display()),
        ))
    }
}

/// Copies `path` to the first free name of `<name>.bak`, `<name>.bak.1`, …
/// beside it. Never replaces an existing file.
fn backup(path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let mut name = path.as_os_str().to_owned();
    name.push(".bak");
    let base = PathBuf::from(name);
    for n in 0u32.. {
        let candidate = if n == 0 {
            base.clone()
        } else {
            let mut s = base.clone().into_os_string();
            s.push(format!(".{n}"));
            PathBuf::from(s)
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                let written = file.write_all(bytes).and_then(|()| file.sync_all());
                if let Err(e) = written {
                    drop(file);
                    let _ = fs::remove_file(&candidate);
                    return Err(e);
                }
                return Ok(candidate);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    unreachable!("u32 backup names exhausted")
}

impl Core {
    #[cfg(test)]
    pub fn with_hooks(hooks: Hooks) -> Core {
        Core {
            hooks,
            ..Core::default()
        }
    }

    pub fn snapshot(&self) -> ScheduleSession {
        let (preview, parse_error) = match self.text.as_ref().map(|t| &t.parsed) {
            Some(Ok(schedule)) => (Some(schedule.preview()), None),
            Some(Err(e)) => (None, Some(e.clone())),
            None => (None, None),
        };
        ScheduleSession {
            text_path: self.text.as_ref().map(|t| display(&t.path)),
            xml_path: self.xml.as_deref().map(display),
            can_apply: preview.is_some() && self.xml.is_some(),
            preview,
            parse_error,
        }
    }

    /// Reads and parses `path`. A parse failure is kept in the session, not
    /// returned; a file that cannot be read leaves the session unchanged.
    pub fn open_text(&mut self, path: &Path) -> Result<(), ScheduleError> {
        require_file(path)?;
        let text = read_text(path, K::TextUnreadable)?;
        let parsed = (self.hooks.parse)(&text).map_err(ScheduleError::from);
        self.text = Some(Text {
            path: path.to_path_buf(),
            parsed,
        });
        Ok(())
    }

    pub fn reload_text(&mut self) -> Result<(), ScheduleError> {
        let path = match &self.text {
            Some(t) => t.path.clone(),
            None => return Err(ScheduleError::new(K::NoText, "no schedule file is open")),
        };
        self.open_text(&path)
    }

    /// Chooses the XML file to update. It is read only when applying.
    pub fn open_xml(&mut self, path: &Path) -> Result<(), ScheduleError> {
        require_file(path)?;
        self.xml = Some(path.to_path_buf());
        Ok(())
    }

    /// Adds the previewed schedule to the XML file as it is on disk now: merges
    /// in memory, saves a backup of the current file, then replaces the file
    /// atomically. Nothing is written when the merge fails.
    pub fn apply(&self) -> Result<ApplySummary, ScheduleError> {
        let text = self
            .text
            .as_ref()
            .ok_or_else(|| ScheduleError::new(K::NoText, "no schedule file is open"))?;
        let schedule = text.parsed.as_ref().map_err(Clone::clone)?;
        let xml_path = self
            .xml
            .as_deref()
            .ok_or_else(|| ScheduleError::new(K::NoXml, "no XML file is chosen"))?;

        let original = fs::read(xml_path).map_err(|e| {
            ScheduleError::new(
                K::XmlUnreadable,
                format!("reading {}: {e}", xml_path.display()),
            )
        })?;
        let xml = std::str::from_utf8(&original).map_err(|_| {
            ScheduleError::new(
                K::XmlUnreadable,
                format!("{} is not UTF-8 text", xml_path.display()),
            )
        })?;
        let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
        let outcome = (self.hooks.merge)(schedule, xml)?;

        let backup_path = backup(xml_path, &original).map_err(|e| {
            ScheduleError::new(
                K::Io,
                format!("saving a backup of {}: {e}", xml_path.display()),
            )
        })?;
        fm_core::atomic_write(xml_path, outcome.xml.as_bytes()).map_err(|e| {
            ScheduleError::new(
                K::Io,
                format!("{e}; the original is unchanged and backed up"),
            )
        })?;
        Ok(ApplySummary {
            xml_path: display(xml_path),
            backup_path: display(&backup_path),
            added: outcome.added,
        })
    }
}
