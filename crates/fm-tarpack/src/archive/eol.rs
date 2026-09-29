//! CRLF to LF conversion as a streaming reader, so the same code counts the
//! converted size and produces the converted bytes.

use std::io::{self, Read};

const DEFAULT_CAPACITY: usize = 64 * 1024;

/// Replaces every `\r\n` with `\n`. A lone `\r` is kept, including one at the
/// very end. A `\r` at the end of one read and `\n` at the start of the next is
/// still one pair.
pub(crate) struct CrlfToLf<R> {
    inner: R,
    buf: Vec<u8>,
    out: Vec<u8>,
    out_pos: usize,
    pending_cr: bool,
    eof: bool,
    /// Pairs replaced so far.
    pub(crate) replaced: u64,
}

impl<R: Read> CrlfToLf<R> {
    pub(crate) fn new(inner: R) -> Self {
        Self::with_capacity(inner, DEFAULT_CAPACITY)
    }

    pub(crate) fn with_capacity(inner: R, capacity: usize) -> Self {
        CrlfToLf {
            inner,
            buf: vec![0; capacity.max(1)],
            out: Vec::new(),
            out_pos: 0,
            pending_cr: false,
            eof: false,
            replaced: 0,
        }
    }

    fn refill(&mut self) -> io::Result<()> {
        self.out.clear();
        self.out_pos = 0;
        while self.out.is_empty() && !self.eof {
            let n = match self.inner.read(&mut self.buf) {
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            if n == 0 {
                self.eof = true;
                if self.pending_cr {
                    self.pending_cr = false;
                    self.out.push(b'\r');
                }
                break;
            }
            for &b in &self.buf[..n] {
                if self.pending_cr {
                    self.pending_cr = false;
                    if b == b'\n' {
                        self.replaced += 1;
                        self.out.push(b'\n');
                        continue;
                    }
                    self.out.push(b'\r');
                }
                if b == b'\r' {
                    self.pending_cr = true;
                } else {
                    self.out.push(b);
                }
            }
        }
        Ok(())
    }
}

impl<R: Read> Read for CrlfToLf<R> {
    fn read(&mut self, dst: &mut [u8]) -> io::Result<usize> {
        if dst.is_empty() {
            return Ok(0);
        }
        if self.out_pos == self.out.len() {
            self.refill()?;
        }
        let n = (self.out.len() - self.out_pos).min(dst.len());
        dst[..n].copy_from_slice(&self.out[self.out_pos..self.out_pos + n]);
        self.out_pos += n;
        Ok(n)
    }
}

/// The converted length and the number of CRLF pairs replaced, in one
/// streaming pass.
pub(crate) fn converted_len<R: Read>(reader: R) -> io::Result<(u64, u64)> {
    let mut conv = CrlfToLf::new(reader);
    let len = io::copy(&mut conv, &mut io::sink())?;
    Ok((len, conv.replaced))
}
