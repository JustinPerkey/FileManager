//! The per-format encoder and decoder.

use std::io::{self, BufRead, Read, Write};

use crate::format::{ArchiveFormat, GZIP_LEVEL, XZ_PRESET, ZSTD_LEVEL, ZSTD_WINDOW_LOG};

/// The tar stream's encoder. The target is a low-power armv7. For zstd and xz
/// the target's decompression memory is set by the window or dictionary, not
/// the level: zstd with an 8 MiB window needs about 8 MiB, xz preset 6 about
/// 9 MiB, gzip 32 KiB. Compression time is paid on the Windows desktop, so the
/// levels are high but the windows are bounded.
pub(crate) enum Encoder<W: Write> {
    Plain(W),
    Gz(flate2::write::GzEncoder<W>),
    Zst(zstd::stream::write::Encoder<'static, W>),
    Xz(liblzma::write::XzEncoder<W>),
}

impl<W: Write> Encoder<W> {
    pub(crate) fn new(format: ArchiveFormat, w: W) -> io::Result<Self> {
        Ok(match format {
            ArchiveFormat::Tar => Encoder::Plain(w),
            // Pure-Rust miniz_oxide backend, fixed 32 KiB window.
            ArchiveFormat::TarGz => Encoder::Gz(flate2::write::GzEncoder::new(
                w,
                flate2::Compression::new(GZIP_LEVEL),
            )),
            ArchiveFormat::TarZst => {
                let mut e = zstd::stream::write::Encoder::new(w, ZSTD_LEVEL)?;
                // Explicit 8 MiB window: bounds the target's memory. Long
                // distance matching stays off (it raises the window), and
                // compression is single-threaded (the default, no workers).
                e.window_log(ZSTD_WINDOW_LOG)?;
                e.long_distance_matching(false)?;
                // A checksum lets the target notice a corrupted archive.
                e.include_checksum(true)?;
                Encoder::Zst(e)
            }
            // Preset 6 (8 MiB dictionary), default check CRC64, one thread.
            ArchiveFormat::TarXz => Encoder::Xz(liblzma::write::XzEncoder::new(w, XZ_PRESET)),
        })
    }

    /// Writes the end frame and returns the underlying writer. A zstd or xz
    /// stream without it is truncated, so the error matters.
    pub(crate) fn finish(self) -> io::Result<W> {
        match self {
            Encoder::Plain(w) => Ok(w),
            Encoder::Gz(e) => e.finish(),
            Encoder::Zst(e) => e.finish(),
            Encoder::Xz(e) => e.finish(),
        }
    }
}

impl<W: Write> Write for Encoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Encoder::Plain(w) => w.write(buf),
            Encoder::Gz(e) => e.write(buf),
            Encoder::Zst(e) => e.write(buf),
            Encoder::Xz(e) => e.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Encoder::Plain(w) => w.flush(),
            Encoder::Gz(e) => e.flush(),
            Encoder::Zst(e) => e.flush(),
            Encoder::Xz(e) => e.flush(),
        }
    }
}

/// Decodes exactly one stream (one gzip member, one zstd frame, one xz
/// stream), so bytes after it can be detected through [`Decoder::into_inner`].
pub(crate) enum Decoder<R: BufRead> {
    Plain(R),
    Gz(flate2::bufread::GzDecoder<R>),
    Zst(zstd::stream::read::Decoder<'static, R>),
    Xz(liblzma::bufread::XzDecoder<R>),
}

impl<R: BufRead> Decoder<R> {
    pub(crate) fn new(format: ArchiveFormat, r: R) -> io::Result<Self> {
        Ok(match format {
            ArchiveFormat::Tar => Decoder::Plain(r),
            ArchiveFormat::TarGz => Decoder::Gz(flate2::bufread::GzDecoder::new(r)),
            ArchiveFormat::TarZst => {
                Decoder::Zst(zstd::stream::read::Decoder::with_buffer(r)?.single_frame())
            }
            ArchiveFormat::TarXz => Decoder::Xz(liblzma::bufread::XzDecoder::new(r)),
        })
    }

    pub(crate) fn into_inner(self) -> R {
        match self {
            Decoder::Plain(r) => r,
            Decoder::Gz(d) => d.into_inner(),
            Decoder::Zst(d) => d.finish(),
            Decoder::Xz(d) => d.into_inner(),
        }
    }
}

impl<R: BufRead> Read for Decoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Decoder::Plain(r) => r.read(buf),
            Decoder::Gz(d) => d.read(buf),
            Decoder::Zst(d) => d.read(buf),
            Decoder::Xz(d) => d.read(buf),
        }
    }
}
