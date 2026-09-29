# M4 — Archive writer

Status: awaiting approval
Project: tarpack   Depends on: M2 (landed), M3 (landed)

## Goal

Given a validated manifest, a Windows source file for every entry, and a chosen
output format, write a correct archive atomically, verify it, and report a
summary. The archive stores absolute names and is uncompressed, gzip, zstd, or
xz.

## Context

What earlier tasks provide:

- **M3** provides `fm_tarpack::manifest`. `Manifest` and `Entry` are already
  validated, with defaults resolved: target dir, target name,
  `target_path()`, mode, uid, gid, uname, gname, `normalize_eol`, plus the
  manifest's `dir_mode` and default owner. The format, the archive layout, the
  output formats, and the extraction commands are documented in
  `docs/tarpack-manifest.md`. Read it.
- **M3** also provides `fm_tarpack::format`: `ArchiveFormat { Tar, TarGz,
  TarZst, TarXz }`, `extension()`, `extract_command(file_name)`, and the
  decided constants `GZIP_LEVEL = 6`, `ZSTD_LEVEL = 19`,
  `ZSTD_WINDOW_LOG = 23`, and `XZ_PRESET = 6`. Use the constants; do not
  introduce other levels.
- **M2** provides `fm_core::atomic_write` and `FmError`.

**Assignments.** M5 builds the real drop and pick logic in parallel with this
task. Define the minimal input type here, in
`crates/fm-tarpack/src/sources/assignments.rs`:
`Assignments(BTreeMap<String /* entry id */, PathBuf>)`. Give it `new`,
`insert`, `get`, and `remove`, and nothing more. M5 extends this same type; it
does not replace it. If M5 landed first and the type exists, use it as is.

### Archive semantics

These are fixed decisions (made by the human, or recorded by the planner with
reasons). Implement them exactly.

- **Order.** Entries go in manifest order. Before each file, emit a directory
  entry for every ancestor of its target directory that has not been emitted
  yet, shallowest first. Each directory is emitted once. The root `/` itself is
  **never** emitted.
- **Directory entries** have mode `dir_mode`, the default uid, gid, uname, and
  gname, and an mtime equal to the newest source mtime in the archive.
- **Names are absolute** (the human's decision). A file is stored as
  `/opt/gateway/bin/gateway`, and its directories as `/opt/`, `/opt/gateway/`,
  and `/opt/gateway/bin/`. Directory names end in `/`. The target extracts
  with GNU tar `-P`.
- **Header format is GNU**, built with `tar::Header::new_gnu()`. `tar`'s
  `Header::set_path` and `Builder::append_data` **reject absolute paths**, so:
  - Write the name bytes directly into the header's 100-byte `name` field
    (`header.as_old_mut().name` or `as_gnu_mut()`), NUL-padded.
  - When the name is **100 bytes or longer**, first emit a GNU long-name
    record:
    - a header with name `././@LongLink`, typeflag `L` (`EntryType::GNULongName`),
      mode `0o644`, uid and gid 0, mtime 0, and size = name length + 1;
    - followed by the name bytes plus one NUL, padded to 512 bytes.

    Then emit the real header with the first 99 bytes of the name in its
    `name` field. This is what GNU tar itself writes.
  - Do **not** use the ustar `prefix`/`name` split. GNU headers reuse those
    bytes, and mixing formats yields headers that some readers misparse.
  - Append with `Builder::append(&header, reader)`, which does not re-validate
    the path. Set the remaining fields through the header's setters, then
    `set_cksum()`.
- **File entries** take their mode, uid, gid, uname, and gname from the manifest
  entry, **never** from the Windows file, which has no Unix mode. `mtime` is the
  source file's modification time in whole seconds.
- **Bytes.**
  - Default: the size and bytes are copied verbatim.
  - Entries with `normalize_eol = true` (the human's decision): every CRLF
    pair (`\r\n`) becomes LF. A lone `\r` is kept. The tar header needs the
    size before the data, so:
    1. Do a streaming counting pass to get the converted size. Handle a `\r`
       at the end of one read buffer followed by `\n` at the start of the next.
    2. Write the header with that size.
    3. Do a second streaming pass that converts while writing.
    4. If the second pass yields a different byte count, the source changed
       mid-build. Fail with `BuildError::SourceChanged { id }`.

    Do not read whole files into memory.

### Output formats

The caller passes an `ArchiveFormat`. The tar stream is written through an
encoder chosen by format:

| Format | Encoder | Settings |
| --- | --- | --- |
| `Tar` | none | — |
| `TarGz` | `flate2::write::GzEncoder` (default pure-Rust `miniz_oxide` backend) | `Compression::new(GZIP_LEVEL)` |
| `TarZst` | `zstd::stream::write::Encoder` | level `ZSTD_LEVEL`; set `window_log(ZSTD_WINDOW_LOG)` explicitly; `include_checksum(true)`; long-distance matching **off**; single-threaded |
| `TarXz` | `liblzma::write::XzEncoder` | `XzEncoder::new(w, XZ_PRESET)`, whose default check is CRC64; single-threaded |

- **Why these settings** (record them in doc comments beside the encoder
  code). The target is a low-power armv7. For zstd and xz, decompression memory
  is set by the window or dictionary, not the level:
  - zstd with an 8 MiB window needs about 8 MiB;
  - xz preset 6 needs about 9 MiB;
  - gzip needs 32 KiB.

  Compression time is paid on the Windows desktop.
- **Crates.** Use `liblzma`, the maintained fork of `xz2` with the same API,
  statically building its bundled source (enable the crate feature that forces
  the bundled/static build). Fall back to `xz2` only if `liblzma` fails to
  build on either CI job, and state the fallback in your handoff. `zstd`
  builds its bundled libzstd by default.
- **C toolchain.** Both need a C compiler: MSVC on Windows (already present for
  Tauri) and `cc` on Linux (preinstalled on `ubuntu-latest`). If a CI job lacks
  one, add the install step to `.github/workflows/ci.yml` and state it in
  `CLAUDE.md`'s Commands prerequisites.
- **Finishing matters.** Call `Builder::into_inner()`, then the encoder's
  `finish()` (or `try_finish`), and handle the error. A zstd or xz stream
  without its end frame is truncated. Then flush and `sync_all` the file.

**Rules that bind this task.**

- Destructive operations are never silent. An existing output file is replaced
  only when the caller passes `overwrite = true`. A failure part way through
  leaves no output file and no temp file, and names the entry that failed.
- Tests never touch real user files; everything happens in a `TempDir`.
- **No test ever runs `tar` with `-P` in extract mode.** That would write to the
  real `/`. System-tar tests list with `-tPf` (read-only), or extract
  **without** `-P` into a temp dir with `-C`. GNU tar then strips the leading
  `/` and prints a notice.
- The crate has no tauri dependency.
- Types that cross to the UI derive `ts_rs::TS`.
- **No `bigint` crosses the boundary.** ts-rs generates `u64`/`i64` as TS
  `bigint`, which the UI cannot use as a number. Every `u64` (or `usize`/`i64`)
  field in an exported type carries `#[ts(type = "number")]`. Values stay far
  below 2^53. That covers `BuildSummary.{entries, files, dirs, bytes,
  uncompressed_bytes}`, `NormalizedEntry.crlf_replaced`, and
  `Progress.{bytes_done, bytes_total}`.

## Files

- `crates/fm-tarpack/src/archive/{mod.rs, plan.rs, header.rs, write.rs, encode.rs, eol.rs, verify.rs}`:
  - `header.rs` handles absolute-name headers and long-name records;
  - `encode.rs` handles the per-format encoder and decoder;
  - `eol.rs` handles CRLF counting and conversion.
- `crates/fm-tarpack/src/sources/{mod.rs, assignments.rs}`: the minimal type
  above
- `crates/fm-tarpack/Cargo.toml`: adds `tar`, `sha2`, `flate2`, `zstd`, and
  `liblzma`. `tempfile` is already present or allowed.
- `.github/workflows/ci.yml`, only if a C compiler install step is needed
- Regenerated TS types

Do not change the manifest or format modules, except to add getters you need.

## Design

- **`ArchivePlan::new(&Manifest, &Assignments) -> Result<ArchivePlan, PlanError>`**
  - A pure function returning the ordered list of `PlannedEntry::Dir { path }`
    and `PlannedEntry::File { id, source, path, mode, owner, normalize_eol }`.
    `path` is the absolute stored name.
  - `PlanError::Unassigned(Vec<id>)` lists every entry without a source.
- **`write_archive(plan, out_path, format, overwrite, progress: impl FnMut(Progress)) -> Result<BuildSummary, BuildError>`**
  1. If `out_path` exists and `overwrite` is false, return
     `BuildError::OutputExists` before touching anything.
  2. Stat every source first, and fail fast with `SourceMissing { id, path }` or
     `SourceUnreadable { id, path, cause }`. For `normalize_eol` entries, run
     the counting pass here, so the total size is known.
  3. Stream the tar through the format's encoder into a `NamedTempFile` in
     `out_path`'s directory. Call `progress` with
     `Progress { phase: Writing, entry_id, bytes_done, bytes_total }` at least
     once per file entry. The byte counts are **uncompressed** tar-stream
     bytes, so they mean the same thing for every format. `bytes_total` is the
     exact stream size, including headers, long-name records, padding, and the
     end-of-archive blocks. It is known after step 2.
  4. Finish the archive and the encoder, flush, and sync.
  5. **Verify.** Re-open the temp file, wrap it in the format's decoder, and
     read it with `tar::Archive`. Check that the entry sequence equals the
     plan. Compare paths with `entry.path_bytes()` (raw bytes, long names
     resolved; do not use `path()`, which may normalise). Also compare modes
     (`& 0o7777`), uid, gid, uname, gname, and sizes. Report progress with
     `phase: Verifying`. On a mismatch, or a decoder error such as a truncated
     stream or bad checksum, return `BuildError::VerifyFailed(detail)`. Also
     fail if raw bytes remain after the compressed stream ends.
     **Compute the SHA-256 in this pass:** tee the raw (compressed) file bytes
     through the hasher as the decoder reads them, and hash any remainder to
     EOF. After verification succeeds, only persisting remains, so no work
     goes unreported.
  6. Persist the temp file over `out_path` (a rename). The hash from step 5 is
     the hash of the persisted file.
  7. Return `BuildSummary`:
     - `path` and `format`;
     - `entries`, `files`, and `dirs`;
     - `bytes`: the size of the output file on disk;
     - `uncompressed_bytes`: the size of the tar stream;
     - `sha256_hex`;
     - `extract_command`: `format.extract_command(<output file name, display string>)`;
     - `normalized_entries: Vec<NormalizedEntry { id, crlf_replaced: u64 }>`,
       listing every `normalize_eol` entry, including those with 0
       replacements.
- `Progress { phase: BuildPhase /* Writing | Verifying */, entry_id: Option<String>, bytes_done: u64, bytes_total: u64 }`.
  The rules are a UI contract; implement them exactly:
  - Both phases use the same `bytes_total`, the uncompressed stream size.
    Within a phase, `bytes_done` never decreases. The verifying phase starts
    again from 0.
  - `entry_id` is the id of the file entry whose bytes are being written or
    checked. It is `None` while directory records, long-name records, or the
    end-of-archive blocks are processed.
  - Each phase ends with exactly one final event, where
    `bytes_done == bytes_total` and `entry_id` is `None`.
  - No event is sent after the final verifying event.
  - On failure, events stop at the point of failure.
  - There is no third phase.
  `BuildSummary`, `NormalizedEntry`, `Progress`, and `BuildPhase` derive
  `ts_rs::TS` with camelCase field names.
- `BuildError` variants: `OutputExists`, `SourceMissing`, `SourceUnreadable`,
  `SourceChanged { id }`, `Io`, `VerifyFailed`. Each carries the entry id where
  there is one.

## Acceptance criteria

- For every format, the archive round-trips exact names, modes, owners, mtimes,
  sizes, and bytes.
- **Every stored name begins with `/`**, including names of 100 bytes or more,
  in both the long-name record and the resolved `path_bytes()`.
- Parent directories appear once, before their children, with `dir_mode`. `/`
  is never an entry.
- `normalize_eol` entries have every CRLF replaced by LF, lone CRs kept, and a
  header size equal to the converted length. Other entries are byte-identical
  to their source, CRLFs included.
- A missing or unreadable source, a source whose size changes between passes,
  or an injected mid-write failure leaves no output file and no temp file, and
  the error names the entry.
- `overwrite = false` never modifies an existing file.
- **On verification failure, nothing is saved at `out_path`.** If a file
  already existed there, even with `overwrite = true`, it is left
  byte-for-byte unchanged, including its modification time. No temp file
  remains. The UI tells the user exactly this, so it must hold.
- Progress events follow the rules in the Design section: final events per
  phase, `None` ids for non-file records, and nothing after verifying ends.
- The generated TS for `BuildSummary`, `NormalizedEntry`, and `Progress` uses
  `number`, never `bigint`.
- A mode the Windows source might suggest never leaks into the header.
- A truncated compressed output (encoder not finished) is caught by
  verification.
- Both CI jobs build the C-backed crates.

## Tests proving completion

`cargo test -p fm-tarpack archive`. All tests run in a `TempDir`, with fixture
manifests built through M3's `parse`:

- `round_trip_preserves_headers`: parameterised over all four formats
- `stored_names_are_absolute`: every entry's `path_bytes()` starts with `b'/'`
- `long_absolute_names_use_gnu_longlink`: a name over 100 bytes, and one of
  exactly 100 bytes. Assert that the raw stream contains a `././@LongLink`
  record of typeflag `L`, and that the resolved name starts with `/`.
- `root_dir_never_emitted`
- `parent_dirs_emitted_once_in_order`
- `shared_parent_dirs_not_duplicated`
- `unassigned_entries_listed`
- `missing_source_writes_nothing`
- `existing_output_requires_overwrite`
- `overwrite_replaces_atomically`
- `summary_hash_matches_file`
- `summary_reports_compressed_and_uncompressed_sizes`
- `extract_command_matches_format`
- `modes_come_from_manifest_not_source`
- `eol_normalized_only_when_opted_in`: CRLF, lone CR, and trailing CR
  fixtures, plus a CRLF split across the read-buffer boundary
- `eol_source_change_between_passes_fails`: drive the second pass through an
  injected reader that yields a different length, and assert `SourceChanged`
  and no output
- `truncated_stream_fails_verification`: for zstd and xz, drop the encoder
  without finishing through a test hook, and assert `VerifyFailed`
- `verify_failure_leaves_existing_output_unchanged`:
  1. Create an existing file at `out_path` with known bytes, and record its
     mtime.
  2. Force verification to fail through a test hook (for example, corrupt
     one byte of the temp file before step 5, or inject a verifier mismatch).
  3. Call with `overwrite = true`, and assert:
     - `VerifyFailed` is returned;
     - `out_path` has the original bytes and mtime;
     - there are no stray files in the directory.

  Repeat with no pre-existing file, and assert that `out_path` does not exist
  afterwards.
- `progress_events_follow_contract`: capture every `Progress` for a manifest
  with nested directories and two files, and assert the phase order, the
  monotonic `bytes_done`, `None` ids on directory records, exactly one final
  event per phase, and nothing after the final verifying event
- `summary_hash_computed_during_verify_matches_file`: the summary hash equals
  a fresh SHA-256 of the persisted file, for all four formats
- `generated_types_use_number`: the generated `BuildSummary.ts`, `Progress.ts`,
  and `NormalizedEntry.ts` contain no `bigint`
- `zstd_frame_window_is_bounded`: read the frame header of the output (for
  example with `zstd::zstd_safe` frame parameters, or by parsing the
  Window_Descriptor), and assert a window of at most 8 MiB (2^23)
- `system_tar_lists_absolute_names`: `#[cfg(unix)]`. Run `tar -tPf <out>`
  (read-only) and assert that every listed name starts with `/`.
- `extracts_with_system_tar_preserving_modes`: `#[cfg(unix)]`. Run
  `tar -xpf <out> -C <tmp>` **without `-P`**, so that the leading `/` is
  stripped into the temp dir, and check the resulting `stat` modes. Run it for
  `.tar`, and for `.tar.gz`, `.tar.xz`, and `.tar.zst` where the system tar
  supports them (probe `tar --zstd --version` and skip with a printed note if
  it is unsupported).

Also run `cargo clippy -p fm-tarpack --all-targets -- -D warnings`, and confirm
both CI jobs are green.

## Out of scope

- Drop matching and remembered locations or format (M5).
- Tauri commands and threading, output-extension handling (M6).
- Multithreaded compression, zstd long mode, xz presets above 6, other formats.
- Static CRT linking for the release exe (M7).

## Risks

- `tar::Builder` writes mtime as seconds; truncate consistently in both the
  write and the verify step.
- On Windows, the temp file must be closed before `persist`. Drop the encoder
  and the builder first.
- Hand-built headers must still pass `set_cksum()`. Forgetting it produces
  archives GNU tar rejects with "checksum error". The system-tar tests catch
  this on Linux.
- The `tar` crate's reader may reject or rewrite absolute names in some APIs.
  Use only `path_bytes()` in verification, never `unpack`.
- `zstd`/`liblzma` add C build time to CI. Cache the cargo target directory.
- A zstd level of 19 compresses at a few MB/s. That is acceptable for the
  expected package sizes, but a very large package will take noticeably long.
  That is why progress is reported.
