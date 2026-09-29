//! Archive output formats: the enum shared by the writer and the remembered
//! state, plus pure helpers and the compression parameters.

use std::ffi::{OsStr, OsString};

use serde::{Deserialize, Serialize};

/// gzip level. Conventional default. gzip's window is fixed at 32 KiB, so the
/// target's decompression cost is negligible.
pub const GZIP_LEVEL: u32 = 6;

/// zstd level. The target is a low-power armv7. zstd's decompression memory is
/// set by the window, not the level. Long mode and levels 20-22 are excluded,
/// because they raise the window.
pub const ZSTD_LEVEL: i32 = 19;

/// zstd window log (2^23 = 8 MiB). Bounds the target's decompression memory
/// and stays within the decoder's default limit.
pub const ZSTD_WINDOW_LOG: u32 = 23;

/// xz preset. xz's default: an 8 MiB dictionary and about 9 MiB to
/// decompress. Presets 7-9 need 17-65 MiB and are excluded.
pub const XZ_PRESET: u32 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveFormat {
    Tar,
    TarGz,
    TarZst,
    TarXz,
}

/// Known suffixes, longest first so the longest match wins.
const SUFFIXES: [(&str, ArchiveFormat); 5] = [
    (".tar.zst", ArchiveFormat::TarZst),
    (".tar.gz", ArchiveFormat::TarGz),
    (".tar.xz", ArchiveFormat::TarXz),
    (".tar", ArchiveFormat::Tar),
    (".tgz", ArchiveFormat::TarGz),
];

/// Returns the known suffix `name` ends with, ASCII-case-insensitively.
fn known_suffix(name: &OsStr) -> Option<(usize, ArchiveFormat)> {
    let bytes = name.as_encoded_bytes();
    SUFFIXES.iter().find_map(|(suffix, format)| {
        let s = suffix.as_bytes();
        (bytes.len() >= s.len() && bytes[bytes.len() - s.len()..].eq_ignore_ascii_case(s))
            .then_some((s.len(), *format))
    })
}

impl ArchiveFormat {
    pub const ALL: [ArchiveFormat; 4] = [
        ArchiveFormat::Tar,
        ArchiveFormat::TarGz,
        ArchiveFormat::TarZst,
        ArchiveFormat::TarXz,
    ];

    pub fn extension(self) -> &'static str {
        match self {
            ArchiveFormat::Tar => ".tar",
            ArchiveFormat::TarGz => ".tar.gz",
            ArchiveFormat::TarZst => ".tar.zst",
            ArchiveFormat::TarXz => ".tar.xz",
        }
    }

    /// The last suffix without its dot: the form a native Save-dialog filter
    /// takes (Windows matches only the last suffix).
    pub fn filter_extension(self) -> &'static str {
        match self {
            ArchiveFormat::Tar => "tar",
            ArchiveFormat::TarGz => "gz",
            ArchiveFormat::TarZst => "zst",
            ArchiveFormat::TarXz => "xz",
        }
    }

    /// Recognises `.tar`, `.tar.gz`, `.tgz`, `.tar.zst` and `.tar.xz`,
    /// ASCII-case-insensitively; the longest suffix wins.
    pub fn from_file_name(name: &OsStr) -> Option<ArchiveFormat> {
        known_suffix(name).map(|(_, f)| f)
    }

    /// Strips a known archive suffix if there is one, then appends this
    /// format's extension. Works on the encoded bytes, never lossily.
    pub fn with_extension(self, name: &OsStr) -> OsString {
        let bytes = name.as_encoded_bytes();
        let keep = match known_suffix(name) {
            Some((len, _)) => &bytes[..bytes.len() - len],
            None => bytes,
        };
        // SAFETY: `keep` is `name`'s encoded bytes cut either at the end or
        // right before an ASCII suffix, which is a valid boundary.
        let mut out = unsafe { OsString::from_encoded_bytes_unchecked(keep.to_vec()) };
        out.push(self.extension());
        out
    }

    /// The exact GNU tar command that extracts the archive on the target.
    /// `file_name` is single-quoted when it has anything outside
    /// `[A-Za-z0-9._+-]`.
    pub fn extract_command(self, file_name: &str) -> String {
        let flag = match self {
            ArchiveFormat::Tar => "",
            ArchiveFormat::TarGz => "-z ",
            ArchiveFormat::TarZst => "--zstd ",
            ArchiveFormat::TarXz => "-J ",
        };
        format!(
            "tar {flag}--no-overwrite-dir -xpPf {}",
            shell_quote(file_name)
        )
    }
}

fn shell_quote(s: &str) -> String {
    let safe = !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'));
    if safe {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(s: &str) -> Option<ArchiveFormat> {
        ArchiveFormat::from_file_name(OsStr::new(s))
    }

    #[test]
    fn format_from_file_name_longest_suffix_wins() {
        assert_eq!(f("a.tar.gz"), Some(ArchiveFormat::TarGz));
        assert_eq!(f("A.TGZ"), Some(ArchiveFormat::TarGz));
        assert_eq!(f("a.tar.zst"), Some(ArchiveFormat::TarZst));
        assert_eq!(f("a.tar.xz"), Some(ArchiveFormat::TarXz));
        assert_eq!(f("a.tar"), Some(ArchiveFormat::Tar));
        assert_eq!(f("a.zip"), None);
        assert_eq!(f("a"), None);
    }

    #[test]
    fn with_extension_replaces_known_suffix_or_appends() {
        let w = |fmt: ArchiveFormat, s: &str| fmt.with_extension(OsStr::new(s));
        assert_eq!(w(ArchiveFormat::TarZst, "x.tar.gz"), "x.tar.zst");
        assert_eq!(w(ArchiveFormat::Tar, "x.TGZ"), "x.tar");
        assert_eq!(w(ArchiveFormat::TarXz, "x"), "x.tar.xz");
        assert_eq!(w(ArchiveFormat::TarGz, "x.zip"), "x.zip.tar.gz");
        assert_eq!(w(ArchiveFormat::TarGz, "a.b.tar"), "a.b.tar.gz");
    }

    #[test]
    fn filter_extension_is_last_suffix_without_dot() {
        let got: Vec<_> = ArchiveFormat::ALL
            .iter()
            .map(|f| f.filter_extension())
            .collect();
        assert_eq!(got, ["tar", "gz", "zst", "xz"]);
        for fmt in ArchiveFormat::ALL {
            assert!(fmt
                .extension()
                .ends_with(&format!(".{}", fmt.filter_extension())));
        }
    }

    #[test]
    fn extract_command_per_format_includes_absolute_names_flag() {
        assert_eq!(
            ArchiveFormat::Tar.extract_command("a.tar"),
            "tar --no-overwrite-dir -xpPf a.tar"
        );
        assert_eq!(
            ArchiveFormat::TarGz.extract_command("a.tar.gz"),
            "tar -z --no-overwrite-dir -xpPf a.tar.gz"
        );
        assert_eq!(
            ArchiveFormat::TarZst.extract_command("a.tar.zst"),
            "tar --zstd --no-overwrite-dir -xpPf a.tar.zst"
        );
        assert_eq!(
            ArchiveFormat::TarXz.extract_command("a.tar.xz"),
            "tar -J --no-overwrite-dir -xpPf a.tar.xz"
        );
    }

    #[test]
    fn extract_command_quotes_unsafe_names() {
        assert_eq!(
            ArchiveFormat::Tar.extract_command("my file.tar"),
            "tar --no-overwrite-dir -xpPf 'my file.tar'"
        );
        assert_eq!(
            ArchiveFormat::Tar.extract_command("it's.tar"),
            "tar --no-overwrite-dir -xpPf 'it'\\''s.tar'"
        );
    }

    #[test]
    fn serde_names_are_camel_case() {
        assert_eq!(
            serde_json_like(ArchiveFormat::TarGz),
            "tarGz",
            "wire name must match the generated TS union"
        );
    }

    fn serde_json_like(f: ArchiveFormat) -> String {
        // No serde_json dependency: round-trip through toml.
        #[derive(Serialize)]
        struct W {
            f: ArchiveFormat,
        }
        let s = toml::to_string(&W { f }).unwrap();
        s.trim()
            .trim_start_matches("f = ")
            .trim_matches('"')
            .to_string()
    }
}
