//! Touches one cheap public item of each production domain crate so the
//! linker keeps them (and their C code: zstd, liblzma, SQLite) in the exe.

use std::fmt::Write as _;

/// One line per crate. `Database::open` on a non-database file really runs
/// SQLite's C code and returns an error.
#[must_use]
pub fn link_report() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "fm-core: to_hex = {}", fm_core::to_hex(&[0xAB, 0xCD]));
    let _ = writeln!(
        out,
        "fm-tarpack: {} archive formats, zstd level {}, xz preset {}",
        fm_tarpack::ArchiveFormat::ALL.len(),
        fm_tarpack::format::ZSTD_LEVEL,
        fm_tarpack::format::XZ_PRESET
    );
    let _ = writeln!(
        out,
        "fm-schedule: parse(\"\") -> {}",
        if fm_schedule::parse("").is_ok() {
            "ok"
        } else {
            "err"
        }
    );
    let root = std::env::temp_dir();
    let _ = writeln!(
        out,
        "fm-pipeline: subdirectories(temp) -> {}",
        fm_pipeline::subdirectories(&root)
            .map_or_else(|e| format!("err {e}"), |d| format!("{} dirs", d.len()))
    );
    let _ = writeln!(out, "fm-dbmanager: {}", sqlite_probe());
    out
}

fn sqlite_probe() -> String {
    let Ok(dir) = tempfile::tempdir() else {
        return "no temp dir".into();
    };
    let file = dir.path().join("not-a-database.db");
    if std::fs::write(
        &file,
        b"this is not a SQLite database, just text padding.....",
    )
    .is_err()
    {
        return "write failed".into();
    }
    match fm_dbmanager::Database::open(&file) {
        Ok(_) => "opened (unexpected)".into(),
        Err(e) => format!("open of a text file -> error (expected): {e}"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn link_report_covers_all_five_crates() {
        let r = super::link_report();
        for name in [
            "fm-core",
            "fm-tarpack",
            "fm-schedule",
            "fm-pipeline",
            "fm-dbmanager",
        ] {
            assert!(r.contains(name), "{name} missing from:\n{r}");
        }
    }
}
