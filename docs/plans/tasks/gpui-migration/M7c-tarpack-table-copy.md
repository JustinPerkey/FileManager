# M7c — Move the Tar Packager table's copy into `text.rs`

Status: approved (2026-11-26; from the U10 review)
Project: gpui-migration   Depends on: U10 (landed)

## Goal

All of the entry table's user-facing copy lives in the tool's `text.rs`
(backend-owned), unchanged, and the view imports it. The rule "views never
write copy" then holds for the table too.

## Context

The tool's copy is in `apps/filemanager/src/tools/tarpack/text.rs`, owned by
the backend implementer and unit-tested. U10 (ui-implementer) built the entry
table in `apps/filemanager/src/tools/tarpack/view/`, and put its copy in
`view/entry_cells.rs`:

- the column headings;
- the status words;
- "Browse…" and "Clear";
- the CRLF → LF marker text;
- the functions `summary` (the table's summary line) and `empty_text` (the
  empty and withheld sentences).

This task is a move, not a rewrite.

### What to do

- Move those strings and the two functions into `text.rs`, **unchanged**:
  same text, same signatures. They are pure, so they take the inputs they
  take today (counts, entries), not view types. If one takes a view type,
  change it to the plain data it reads, and say so in the PR.
- Make `view/entry_cells.rs` import them from `super::super::text` (or
  `crate::tools::tarpack::text`). Remove its copies.
- Move the tests of `summary` and `empty_text`, if any live in the view's
  tests, to `tools/tarpack/tests.rs` under the same names.

### Rules that bind this task

- No visible change: the rendered text is identical. The view tests still
  pass unchanged, apart from imports.
- Views call only the model and read copy from `text.rs`.
- fmt (linux job), clippy pedantic `-D warnings`, `cargo test --workspace`.

## Files

Edit: `apps/filemanager/src/tools/tarpack/{text.rs,tests.rs}`, and
`apps/filemanager/src/tools/tarpack/view/entry_cells.rs` (imports and
removals only).

Do not touch: layout, behaviour, other tools, `docs/plans/`.

## Acceptance criteria

- `view/entry_cells.rs` defines no user-facing string literal. Check this
  with a grep for string literals there, ignoring tags and debug selectors.
- Every view and model test passes.

## Tests proving completion

- The moved tests, and all existing tests.

## Out of scope

- Any wording change.
