//! The Schedule Creator's session logic as plain functions, with no Tauri
//! types. The parse and merge steps go through [`Hooks`], which default to
//! `fm_schedule::parse` and `fm_schedule::merge`; tests swap in fakes. Both
//! run only in [`Core::apply`].

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use fm_schedule::{MergeError, MergeOutcome, ParseError, Schedule};

use super::types::{
    ApplySummary, FileSlot, ScheduleError, ScheduleErrorKind as K, ScheduleSession,
};

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

/// A chosen file and, if it cannot be used, why.
struct Slot {
    path: PathBuf,
    error: Option<ScheduleError>,
}

impl Slot {
    /// Checks `path` is a file of UTF-8 text. A problem is kept, not returned.
    fn check(path: &Path, unreadable: K) -> Slot {
        let error = require_file(path)
            .and_then(|()| read_text(path, unreadable))
            .err();
        Slot {
            path: path.to_path_buf(),
            error,
        }
    }

    fn view(&self) -> FileSlot {
        FileSlot {
            path: display(&self.path),
            error: self.error.clone(),
        }
    }

    /// The path, if the slot is usable.
    fn usable<'a>(
        slot: &'a Option<Slot>,
        missing: K,
        what: &str,
    ) -> Result<&'a Path, ScheduleError> {
        match slot {
            None => Err(ScheduleError::new(missing, format!("no {what} is chosen"))),
            Some(Slot { error: Some(e), .. }) => Err(e.clone()),
            Some(Slot { path, error: None }) => Ok(path),
        }
    }
}

#[derive(Default)]
pub(super) struct Core {
    hooks: Hooks,
    text: Option<Slot>,
    xml: Option<Slot>,
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

const BOM: &str = "\u{feff}";

/// Reads `path` as UTF-8 text. Returns the text without a leading byte-order
/// mark, and whether it had one.
fn read_text(path: &Path, kind: K) -> Result<(String, bool), ScheduleError> {
    let bytes = fs::read(path)
        .map_err(|e| ScheduleError::new(kind, format!("reading {}: {e}", path.display())))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| ScheduleError::new(kind, format!("{} is not UTF-8 text", path.display())))?;
    Ok(match text.strip_prefix(BOM) {
        Some(rest) => (rest.to_owned(), true),
        None => (text, false),
    })
}

fn require_file(path: &Path) -> Result<(), ScheduleError> {
    // A relative path would resolve against the process's working directory,
    // which the user never sees.
    if !path.is_absolute() {
        return Err(ScheduleError::new(
            K::NotAbsolute,
            format!("{} is not a full path", path.display()),
        ));
    }
    if path.is_file() {
        Ok(())
    } else {
        Err(ScheduleError::new(
            K::NotAFile,
            format!("{} is not a file", path.display()),
        ))
    }
}

fn is_xml(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
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
        let ok = |s: &Option<Slot>| s.as_ref().is_some_and(|s| s.error.is_none());
        ScheduleSession {
            text: self.text.as_ref().map(Slot::view),
            xml: self.xml.as_ref().map(Slot::view),
            can_apply: ok(&self.text) && ok(&self.xml),
        }
    }

    /// Sets or clears (`None`) the schedule text file. It is checked to be a
    /// readable UTF-8 file, not parsed.
    pub fn set_text(&mut self, path: Option<&Path>) {
        self.text = path.map(|p| Slot::check(p, K::TextUnreadable));
    }

    /// Sets or clears (`None`) the XML file. It is checked to be a readable
    /// UTF-8 file, and read again when applying.
    pub fn set_xml(&mut self, path: Option<&Path>) {
        self.xml = path.map(|p| Slot::check(p, K::XmlUnreadable));
    }

    /// Sets the slots from dropped paths: a `.xml` file is the XML file,
    /// anything else the schedule file. A slot with nothing dropped for it
    /// is kept. More than one path for a slot changes nothing.
    pub fn set_dropped(&mut self, paths: &[PathBuf]) -> Result<(), ScheduleError> {
        let (xml, text): (Vec<&PathBuf>, Vec<&PathBuf>) = paths.iter().partition(|p| is_xml(p));
        if xml.len() > 1 || text.len() > 1 {
            return Err(ScheduleError::new(
                K::DropAmbiguous,
                format!(
                    "{} XML and {} other files were dropped; drop at most one of each",
                    xml.len(),
                    text.len()
                ),
            ));
        }
        if let Some(p) = text.first() {
            self.set_text(Some(p));
        }
        if let Some(p) = xml.first() {
            self.set_xml(Some(p));
        }
        Ok(())
    }

    /// Reads and parses the schedule file, adds it to the XML file as it is on
    /// disk now, saves a backup of the current XML, then replaces it
    /// atomically. Nothing is written when parsing or merging fails.
    ///
    /// `expected_text` and `expected_xml` are the display paths the user
    /// confirmed; if the session holds other files, nothing is written.
    pub fn apply(
        &self,
        expected_text: &str,
        expected_xml: &str,
    ) -> Result<ApplySummary, ScheduleError> {
        let text_path = Slot::usable(&self.text, K::NoText, "schedule file")?;
        let xml_path = Slot::usable(&self.xml, K::NoXml, "XML file")?;
        if display(text_path) != expected_text || display(xml_path) != expected_xml {
            return Err(ScheduleError::new(
                K::FilesChanged,
                "the chosen files changed after the update was confirmed",
            ));
        }

        let (text, _) = read_text(text_path, K::TextUnreadable)?;
        let schedule = (self.hooks.parse)(&text)?;

        let (xml, bom) = read_text(xml_path, K::XmlUnreadable)?;
        let outcome = (self.hooks.merge)(&schedule, &xml)?;
        // The backup holds the original bytes; the new file keeps its BOM.
        let original = if bom { format!("{BOM}{xml}") } else { xml };
        let updated = if bom {
            format!("{BOM}{}", outcome.xml)
        } else {
            outcome.xml
        };

        let backup_path = backup(xml_path, original.as_bytes()).map_err(|e| {
            ScheduleError::new(
                K::Io,
                format!("saving a backup of {}: {e}", xml_path.display()),
            )
        })?;
        fm_core::atomic_write(xml_path, updated.as_bytes()).map_err(|e| {
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
