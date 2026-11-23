# M9b — Example files are LF everywhere (fixes the red Windows CI)

Status: approved (2026-11-23; Windows CI red since 1e0d6cf)
Project: dbmanager (repository-wide fix)   Depends on: none. Run it first; it unblocks Windows CI.

## Goal

Make the `examples/` files check out with LF line endings on every
platform. Tests that `include_str!` them and edit them with `"\n"` patterns
then behave the same on Windows as on Linux.

## Context

Two `fm-dbmanager` tests fail on the Windows CI job:

- `session::tests::links::create_only_link_field`, at
  `assert!(EXAMPLE.contains(from))` (`tests/links.rs`, around line 558);
- `session::tests::links::hidden_required_link_field_blocks_add` (around
  line 622).

`EXAMPLE` is `include_str!("…/examples/dbmanager/example.toml")`. On the
Windows checkout the file has CRLF line endings: the repository has no
`.gitattributes` rule for it, so `core.autocrlf` decides. The tests'
`replacen("…\n…", …)` patterns never match there. `tests/delete.rs` (around
line 174) and other `replacen` calls with `\n` have the same risk. An
earlier fix (an M6 test) normalised CRLF in one test. This fix removes the
cause for every example file instead.

`.gitattributes` today has one line:
`apps/desktop/src/lib/generated/** text eol=lf`.

### What to do

1. Add `examples/** text eol=lf` to `.gitattributes`, with a comment:
   example files are read by tests through `include_str!` and edited with
   `"\n"` patterns, so they must be LF on every checkout.
2. Renormalise: `git add --renormalize examples/`. The index already holds
   LF, so expect no content change; commit any that appears.
3. **Check nothing relies on CRLF input from `examples/`.** Grep every
   crate's tests for `include_str!` of an `examples/` file, and look for any
   that expects `\r\n` in it. The Tar Packager's CRLF → LF tests build their
   CRLF input inline in `fm-tarpack` tests, not from `examples/`. Confirm
   that, and list what you checked in the PR.
4. **Grep the other crates** (`fm-tarpack`, `fm-schedule`, `fm-pipeline`,
   `fm-core`, `apps/filemanager`) for `include_str!` combined with
   `.replace` / `.replacen` / `.contains` using a `\n` pattern on non-example
   files. Any such input that is not under `examples/` either gets a
   `.gitattributes` line too, or its test normalises with
   `.replace("\r\n", "\n")`. List each in the PR.
5. Leave the earlier M6 normalisation in place (harmless), or remove it.
   Say which in the PR.

### Behaviour change to note

`fm_tarpack`'s create-from-example writes `examples/tarpack/example.toml`'s
text. On Windows it now writes LF, not CRLF. TOML accepts both, and editors
on Windows 11 handle LF. Record this in the PR.

### Rules that bind this task

- Tests never touch real user files.
- fmt, clippy pedantic and `cargo test --workspace` pass on **both** CI
  jobs. The Windows job is the proof.

## Files

Edit: `.gitattributes`. Tests only if step 4 finds a non-example input.

## Acceptance criteria

- The Windows CI job is green, with `create_only_link_field` and
  `hidden_required_link_field_blocks_add` passing.
- `git ls-files --eol examples` shows `attr/text eol=lf` for every file.

## Tests proving completion

- The two failing tests, and every test, on the Windows job.

## Out of scope

- Anything else in M9 (its Review follow-up is a separate run).
