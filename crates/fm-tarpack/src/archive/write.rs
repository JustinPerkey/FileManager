//! Writes the plan through the format's encoder into a temp file, verifies it,
//! and persists it over the output path.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use super::encode::Encoder;
use super::eol::{converted_len, CrlfToLf};
use super::header::{append_entry, preamble_size, record_size, Meta, END_BLOCKS};
use super::plan::{ArchivePlan, PlannedEntry};
use super::verify::{verify, Expected};
use crate::format::ArchiveFormat;
use crate::manifest::{Diagnostic, EntryFailure};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum BuildPhase {
    Writing,
    Verifying,
}

/// A progress event. Byte counts are uncompressed tar-stream bytes, so they
/// mean the same thing for every format. Both phases share `bytes_total`;
/// `bytes_done` never decreases within a phase, and the verifying phase starts
/// again from 0. `entry_id` is the file entry being written or checked, and
/// `None` for directory records, long-name records and the end-of-archive
/// blocks. Each phase ends with one event where `bytes_done == bytes_total`
/// and `entry_id` is `None`; nothing follows the final verifying event, and on
/// failure events stop at the point of failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: BuildPhase,
    pub entry_id: Option<String>,
    #[ts(type = "number")]
    pub bytes_done: u64,
    #[ts(type = "number")]
    pub bytes_total: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedEntry {
    pub id: String,
    #[ts(type = "number")]
    pub crlf_replaced: u64,
}

/// The build's result and final report. `left_out`, `manifest_errors` and
/// `warnings` are copied from the parse report; `error_count == 0` means the
/// archive holds every entry the manifest lists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct BuildSummary {
    /// The output path, as a display string.
    pub path: String,
    pub format: ArchiveFormat,
    #[ts(type = "number")]
    pub entries: u64,
    #[ts(type = "number")]
    pub files: u64,
    #[ts(type = "number")]
    pub dirs: u64,
    /// Size of the output file on disk.
    #[ts(type = "number")]
    pub bytes: u64,
    /// Size of the tar stream before compression.
    #[ts(type = "number")]
    pub uncompressed_bytes: u64,
    pub sha256_hex: String,
    pub extract_command: String,
    pub normalized_entries: Vec<NormalizedEntry>,
    /// Ids of the file entries written, in archive order.
    pub built_ids: Vec<String>,
    pub left_out: Vec<EntryFailure>,
    pub manifest_errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
    pub error_count: u32,
}

#[derive(Debug)]
pub enum BuildError {
    OutputExists {
        path: PathBuf,
    },
    SourceMissing {
        id: String,
        path: PathBuf,
    },
    SourceUnreadable {
        id: String,
        path: PathBuf,
        cause: io::Error,
    },
    /// The source changed size while the archive was being written.
    SourceChanged {
        id: String,
    },
    Io {
        id: Option<String>,
        cause: io::Error,
    },
    VerifyFailed(String),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::OutputExists { path } => {
                write!(f, "{} already exists", path.display())
            }
            BuildError::SourceMissing { id, path } => {
                write!(f, "entry `{id}`: {} does not exist", path.display())
            }
            BuildError::SourceUnreadable { id, path, cause } => {
                write!(f, "entry `{id}`: cannot read {}: {cause}", path.display())
            }
            BuildError::SourceChanged { id } => {
                write!(f, "entry `{id}`: the source file changed while building")
            }
            BuildError::Io {
                id: Some(id),
                cause,
            } => write!(f, "entry `{id}`: {cause}"),
            BuildError::Io { id: None, cause } => write!(f, "{cause}"),
            BuildError::VerifyFailed(d) => write!(f, "the archive failed verification: {d}"),
        }
    }
}

impl std::error::Error for BuildError {}

fn io_err(cause: io::Error) -> BuildError {
    BuildError::Io { id: None, cause }
}

/// Test hooks. All off in production. `open` and `fail_write_after` are wired
/// into the production write path (source opening and the counting writer), so
/// they stay compiled in; the post-write tampering hooks are test-only.
#[derive(Default)]
pub(crate) struct Hooks {
    /// Opens a source for a pass (0 = counting, 1 = writing) instead of
    /// `File::open`.
    #[allow(clippy::type_complexity)]
    pub open: Option<Box<dyn Fn(&Path, u8) -> io::Result<Box<dyn Read>>>>,
    /// Chops this many bytes off the finished temp file before verification.
    #[cfg(test)]
    pub truncate_tail: u64,
    /// Flips the byte at this offset of the finished temp file.
    #[cfg(test)]
    pub corrupt_at: Option<u64>,
    /// Fails the tar stream writer once this many bytes were written.
    pub fail_write_after: Option<u64>,
}

struct Counting<W> {
    inner: W,
    count: u64,
    fail_after: Option<u64>,
}

impl<W: Write> Write for Counting<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self
            .fail_after
            .is_some_and(|n| self.count + buf.len() as u64 > n)
        {
            return Err(io::Error::other("injected write failure"));
        }
        let n = self.inner.write(buf)?;
        self.count += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Reads a source, counts its bytes, and reports progress. A read error is
/// kept so it can be told apart from a write error.
struct Tracked<'a, R> {
    inner: R,
    count: u64,
    base: u64,
    total: u64,
    id: &'a str,
    progress: &'a mut dyn FnMut(Progress),
    read_err: Option<io::Error>,
}

impl<R: Read> Read for Tracked<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.inner.read(buf) {
            Ok(n) => {
                if n > 0 {
                    self.count += n as u64;
                    (self.progress)(Progress {
                        phase: BuildPhase::Writing,
                        entry_id: Some(self.id.to_string()),
                        bytes_done: (self.base + self.count).min(self.total),
                        bytes_total: self.total,
                    });
                }
                Ok(n)
            }
            Err(e) => {
                // io::copy retries Interrupted, so it is not a failure.
                if e.kind() != io::ErrorKind::Interrupted {
                    self.read_err = Some(io::Error::new(e.kind(), e.to_string()));
                }
                Err(e)
            }
        }
    }
}

struct Prepared {
    size: u64,
    mtime: u64,
    crlf_replaced: u64,
}

fn open_source(hooks: &Hooks, path: &Path, pass: u8) -> io::Result<Box<dyn Read>> {
    match &hooks.open {
        Some(open) => open(path, pass),
        None => Ok(Box::new(File::open(path)?)),
    }
}

fn source_error(id: &str, path: &Path, e: io::Error) -> BuildError {
    if e.kind() == io::ErrorKind::NotFound {
        BuildError::SourceMissing {
            id: id.to_string(),
            path: path.to_path_buf(),
        }
    } else {
        BuildError::SourceUnreadable {
            id: id.to_string(),
            path: path.to_path_buf(),
            cause: e,
        }
    }
}

fn prepare(
    id: &str,
    path: &Path,
    normalize_eol: bool,
    hooks: &Hooks,
) -> Result<Prepared, BuildError> {
    let meta = fs::metadata(path).map_err(|e| source_error(id, path, e))?;
    if !meta.is_file() {
        return Err(source_error(
            id,
            path,
            io::Error::other("not a regular file"),
        ));
    }
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let reader = open_source(hooks, path, 0).map_err(|e| source_error(id, path, e))?;
    if normalize_eol {
        let (size, crlf_replaced) = converted_len(reader).map_err(|e| source_error(id, path, e))?;
        Ok(Prepared {
            size,
            mtime,
            crlf_replaced,
        })
    } else {
        Ok(Prepared {
            size: meta.len(),
            mtime,
            crlf_replaced: 0,
        })
    }
}

fn name_of(path: &str) -> &[u8] {
    path.as_bytes()
}

/// Writes `plan` to `out_path` in `format`. See the module docs for the steps.
/// A failure leaves no output and no temp file; an existing `out_path` is
/// replaced only when `overwrite` is true and the new archive verified.
pub fn write_archive(
    plan: &ArchivePlan,
    out_path: &Path,
    format: ArchiveFormat,
    overwrite: bool,
    mut progress: impl FnMut(Progress),
) -> Result<BuildSummary, BuildError> {
    write_archive_with(
        plan,
        out_path,
        format,
        overwrite,
        &mut progress,
        &Hooks::default(),
    )
}

pub(crate) fn write_archive_with(
    plan: &ArchivePlan,
    out_path: &Path,
    format: ArchiveFormat,
    overwrite: bool,
    progress: &mut dyn FnMut(Progress),
    hooks: &Hooks,
) -> Result<BuildSummary, BuildError> {
    if !overwrite && out_path.symlink_metadata().is_ok() {
        return Err(BuildError::OutputExists {
            path: out_path.to_path_buf(),
        });
    }

    // Stat every source first, and size the whole stream.
    let mut prepared: Vec<Option<Prepared>> = Vec::with_capacity(plan.entries().len());
    let mut total = END_BLOCKS;
    let mut newest = 0u64;
    for pe in plan.entries() {
        match pe {
            PlannedEntry::Dir { path } => {
                total += record_size(path.len(), 0);
                prepared.push(None);
            }
            PlannedEntry::File {
                id,
                source,
                path,
                normalize_eol,
                ..
            } => {
                let p = prepare(id, source, *normalize_eol, hooks)?;
                total += record_size(path.len(), p.size);
                newest = newest.max(p.mtime);
                prepared.push(Some(p));
            }
        }
    }

    let dir = match out_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    // A recognisable name, so a crash's leftover can be identified.
    let tmp = tempfile::Builder::new()
        .prefix(&temp_prefix(out_path))
        .suffix(".partial")
        .tempfile_in(dir)
        .map_err(io_err)?;

    let mut expected: Vec<Expected> = Vec::new();
    let uncompressed = {
        let sink = BufWriter::new(tmp.as_file());
        let encoder = Encoder::new(format, sink).map_err(io_err)?;
        let mut builder = tar::Builder::new(Counting {
            inner: encoder,
            count: 0,
            fail_after: hooks.fail_write_after,
        });
        let mut pos = 0u64;
        for (pe, prep) in plan.entries().iter().zip(&prepared) {
            match pe {
                PlannedEntry::Dir { path } => {
                    let meta = Meta {
                        kind: tar::EntryType::Directory,
                        mode: plan.dir_mode(),
                        owner: plan.dir_owner(),
                        mtime: newest,
                        size: 0,
                    };
                    append_entry(&mut builder, name_of(path), &meta, &mut io::empty())
                        .map_err(io_err)?;
                    expected.push(expect(path, None, &meta));
                    pos += record_size(path.len(), 0);
                    progress(Progress {
                        phase: BuildPhase::Writing,
                        entry_id: None,
                        bytes_done: pos,
                        bytes_total: total,
                    });
                }
                PlannedEntry::File {
                    id,
                    source,
                    path,
                    mode,
                    owner,
                    normalize_eol,
                } => {
                    let prep = prep.as_ref().expect("files are prepared");
                    let meta = Meta {
                        kind: tar::EntryType::Regular,
                        mode: *mode,
                        owner,
                        mtime: prep.mtime,
                        size: prep.size,
                    };
                    let base = pos + preamble_size(path.len());
                    progress(Progress {
                        phase: BuildPhase::Writing,
                        entry_id: Some(id.clone()),
                        bytes_done: base,
                        bytes_total: total,
                    });
                    let raw =
                        open_source(hooks, source, 1).map_err(|e| source_error(id, source, e))?;
                    let inner: Box<dyn Read> = if *normalize_eol {
                        Box::new(CrlfToLf::new(raw))
                    } else {
                        raw
                    };
                    let mut tracked = Tracked {
                        inner,
                        count: 0,
                        base,
                        total,
                        id,
                        progress: &mut *progress,
                        read_err: None,
                    };
                    // Read one byte past the size, so growth is caught
                    // instead of silently truncated.
                    let result = {
                        let mut r = (&mut tracked as &mut dyn Read).take(prep.size + 1);
                        append_entry(&mut builder, name_of(path), &meta, &mut r)
                    };
                    let read_err = tracked.read_err.take();
                    let count = tracked.count;
                    drop(tracked);
                    if let Some(cause) = read_err {
                        return Err(source_error(id, source, cause));
                    }
                    if let Err(cause) = result {
                        return Err(BuildError::Io {
                            id: Some(id.clone()),
                            cause,
                        });
                    }
                    if count != prep.size {
                        return Err(BuildError::SourceChanged { id: id.clone() });
                    }
                    expected.push(expect(path, Some(id.clone()), &meta));
                    pos += record_size(path.len(), prep.size);
                }
            }
        }
        let counting = builder.into_inner().map_err(io_err)?;
        let count = counting.count;
        let sink = counting.inner.finish().map_err(io_err)?;
        sink.into_inner().map_err(|e| io_err(e.into_error()))?;
        if count != total {
            return Err(io_err(io::Error::other(format!(
                "wrote {count} bytes, planned {total}"
            ))));
        }
        count
    };

    tmp.as_file().sync_all().map_err(io_err)?;
    #[cfg(test)]
    if hooks.truncate_tail > 0 || hooks.corrupt_at.is_some() {
        tamper(&tmp, hooks).map_err(io_err)?;
    }
    progress(Progress {
        phase: BuildPhase::Writing,
        entry_id: None,
        bytes_done: uncompressed,
        bytes_total: total,
    });

    let hash = verify(tmp.path(), format, &expected, total, progress)?;
    let bytes = tmp.as_file().metadata().map_err(io_err)?.len();

    persist(tmp, out_path, overwrite).map_err(|e| {
        if !overwrite && e.kind() == io::ErrorKind::AlreadyExists {
            BuildError::OutputExists {
                path: out_path.to_path_buf(),
            }
        } else {
            io_err(e)
        }
    })?;

    let mut built_ids = Vec::new();
    let mut normalized_entries = Vec::new();
    let mut dirs = 0u64;
    for (pe, prep) in plan.entries().iter().zip(&prepared) {
        match pe {
            PlannedEntry::Dir { .. } => dirs += 1,
            PlannedEntry::File {
                id, normalize_eol, ..
            } => {
                built_ids.push(id.clone());
                if *normalize_eol {
                    normalized_entries.push(NormalizedEntry {
                        id: id.clone(),
                        crlf_replaced: prep.as_ref().map_or(0, |p| p.crlf_replaced),
                    });
                }
            }
        }
    }
    let file_name = out_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(BuildSummary {
        path: out_path.display().to_string(),
        format,
        entries: plan.entries().len() as u64,
        files: built_ids.len() as u64,
        dirs,
        bytes,
        uncompressed_bytes: uncompressed,
        sha256_hex: hash.iter().map(|b| format!("{b:02x}")).collect(),
        extract_command: format.extract_command(&file_name),
        normalized_entries,
        built_ids,
        left_out: plan.left_out().to_vec(),
        manifest_errors: plan.manifest_errors().to_vec(),
        warnings: plan.warnings().to_vec(),
        error_count: plan.error_count(),
    })
}

/// `.<output name>.`, with a long name cut so the temp name (16 bytes longer)
/// stays inside the 255-byte component limit.
fn temp_prefix(out_path: &Path) -> std::ffi::OsString {
    const MAX: usize = 200;
    let name = out_path.file_name().unwrap_or_default();
    let mut prefix = std::ffi::OsString::from(".");
    if name.len() <= MAX {
        prefix.push(name);
    } else {
        let lossy = name.to_string_lossy();
        let mut end = MAX;
        while !lossy.is_char_boundary(end) {
            end -= 1;
        }
        prefix.push(&lossy[..end]);
    }
    prefix.push(".");
    prefix
}

/// Whether a failed rename is worth retrying: another process briefly holds
/// the file. On Windows that is usually ERROR_SHARING_VIOLATION (32) or
/// ERROR_LOCK_VIOLATION (33), which std does not map to `PermissionDenied`.
fn transient(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied
        || (cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)))
}

/// Renames the verified temp file onto `out_path`. On Windows an antivirus
/// scanner or the search indexer can briefly hold a new file open without
/// share-delete, which fails the rename (see `transient`); retry a few
/// times before giving up. Dropping the temp file on failure deletes it.
fn persist(mut tmp: NamedTempFile, out_path: &Path, overwrite: bool) -> io::Result<()> {
    const RETRIES: u32 = 5;
    let mut attempt = 0;
    loop {
        let r = if overwrite {
            tmp.persist(out_path)
        } else {
            tmp.persist_noclobber(out_path)
        };
        match r {
            Ok(_) => return Ok(()),
            Err(e) if transient(&e.error) && attempt < RETRIES => {
                attempt += 1;
                tmp = e.file;
                std::thread::sleep(std::time::Duration::from_millis(100 * u64::from(attempt)));
            }
            Err(e) => return Err(e.error),
        }
    }
}

fn expect(path: &str, id: Option<String>, meta: &Meta<'_>) -> Expected {
    Expected {
        name: name_of(path).to_vec(),
        is_dir: id.is_none(),
        id,
        mode: meta.mode & 0o7777,
        uid: u64::from(meta.owner.uid),
        gid: u64::from(meta.owner.gid),
        uname: meta.owner.uname.clone(),
        gname: meta.owner.gname.clone(),
        size: meta.size,
        mtime: meta.mtime,
    }
}

#[cfg(test)]
fn tamper(tmp: &NamedTempFile, hooks: &Hooks) -> io::Result<()> {
    use std::io::{Seek, SeekFrom};

    let file = tmp.as_file();
    if hooks.truncate_tail > 0 {
        let len = file.metadata()?.len();
        file.set_len(len.saturating_sub(hooks.truncate_tail))?;
    }
    if let Some(at) = hooks.corrupt_at {
        let mut f = tmp.reopen()?;
        f.seek(SeekFrom::Start(at))?;
        let mut b = [0u8; 1];
        f.read_exact(&mut b)?;
        f.seek(SeekFrom::Start(at))?;
        f.write_all(&[b[0] ^ 0xff])?;
        f.sync_all()?;
    }
    Ok(())
}
