//! Re-reads the finished temp file through the format's decoder and checks it
//! against what was written. The SHA-256 of the raw file is computed in the
//! same pass.

use std::cell::Cell;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;
use std::rc::Rc;

use sha2::{Digest, Sha256};

use super::encode::Decoder;
use super::write::{BuildError, BuildPhase, Progress};
use crate::format::ArchiveFormat;

/// What one archive record must look like when read back.
#[derive(Clone, Debug)]
pub(crate) struct Expected {
    pub name: Vec<u8>,
    pub is_dir: bool,
    /// The file entry's id; `None` for directories.
    pub id: Option<String>,
    pub mode: u32,
    pub uid: u64,
    pub gid: u64,
    pub uname: String,
    pub gname: String,
    pub size: u64,
    pub mtime: u64,
}

struct HashingReader<R> {
    inner: R,
    hasher: Sha256,
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }
}

struct CountingReader<R> {
    inner: R,
    count: Rc<Cell<u64>>,
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.count.set(self.count.get() + n as u64);
        Ok(n)
    }
}

fn fail(detail: impl std::fmt::Display) -> BuildError {
    BuildError::VerifyFailed(detail.to_string())
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Returns the SHA-256 of the file's raw bytes.
pub(crate) fn verify(
    path: &Path,
    format: ArchiveFormat,
    expected: &[Expected],
    total: u64,
    progress: &mut dyn FnMut(Progress),
) -> Result<[u8; 32], BuildError> {
    let file = File::open(path).map_err(|e| fail(format!("cannot reopen the output: {e}")))?;
    let raw = BufReader::new(HashingReader {
        inner: file,
        hasher: Sha256::new(),
    });
    let mut decoder = Decoder::new(format, raw).map_err(fail)?;
    let count = Rc::new(Cell::new(0u64));
    let event = |progress: &mut dyn FnMut(Progress), id: Option<String>| {
        progress(Progress {
            phase: BuildPhase::Verifying,
            entry_id: id,
            bytes_done: count.get().min(total),
            bytes_total: total,
        });
    };
    let mut counted = CountingReader {
        inner: &mut decoder,
        count: Rc::clone(&count),
    };

    {
        let mut archive = tar::Archive::new(&mut counted);
        let entries = archive.entries().map_err(fail)?;
        let mut want = expected.iter();
        for entry in entries {
            let mut entry = entry.map_err(fail)?;
            let Some(w) = want.next() else {
                return Err(fail(format!(
                    "unexpected extra entry `{}`",
                    lossy(&entry.path_bytes())
                )));
            };
            let name = entry.path_bytes().into_owned();
            if name != w.name {
                return Err(fail(format!(
                    "entry name `{}` where `{}` was written",
                    lossy(&name),
                    lossy(&w.name)
                )));
            }
            let what = lossy(&name);
            let h = entry.header();
            let wanted = if w.is_dir {
                tar::EntryType::Directory
            } else {
                tar::EntryType::Regular
            };
            if h.entry_type() != wanted {
                return Err(fail(format!("`{what}` has the wrong entry type")));
            }
            let mode = h.mode().map_err(fail)? & 0o7777;
            let uid = h.uid().map_err(fail)?;
            let gid = h.gid().map_err(fail)?;
            let mtime = h.mtime().map_err(fail)?;
            let uname = h.username_bytes().unwrap_or(b"");
            let gname = h.groupname_bytes().unwrap_or(b"");
            if mode != w.mode
                || uid != w.uid
                || gid != w.gid
                || mtime != w.mtime
                || uname != w.uname.as_bytes()
                || gname != w.gname.as_bytes()
            {
                return Err(fail(format!(
                    "`{what}` header fields differ from what was written"
                )));
            }
            let size = entry.size();
            if size != w.size {
                return Err(fail(format!(
                    "`{what}` has size {size}, expected {}",
                    w.size
                )));
            }
            event(progress, w.id.clone());
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                let n = entry.read(&mut buf).map_err(fail)?;
                if n == 0 {
                    break;
                }
                event(progress, w.id.clone());
            }
        }
        if want.next().is_some() {
            return Err(fail("the archive has fewer entries than were written"));
        }
    }

    // The end-of-archive blocks and the encoder's end frame come after the
    // last entry. Draining to EOF surfaces a truncated stream.
    io::copy(&mut counted, &mut io::sink()).map_err(fail)?;
    if count.get() != total {
        return Err(fail(format!(
            "the stream holds {} bytes, expected {total}",
            count.get()
        )));
    }
    let mut rest = decoder.into_inner();
    let mut trailing = Vec::new();
    rest.read_to_end(&mut trailing).map_err(fail)?;
    if !trailing.is_empty() {
        return Err(fail("bytes remain after the end of the compressed stream"));
    }
    let hashing = rest.into_inner();
    event(progress, None);
    Ok(hashing.hasher.finalize().into())
}
