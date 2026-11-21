# M15 — Allow the shorthand classes in config patterns

Status: awaiting approval (new 2026-11-21: the human decided to lift the
shorthand-class ban once only Rust validates; the TypeScript validation
mirror is dropped, so that holds now)
Project: dbmanager   Depends on: M5 (landed)

## Goal

Accept `\d \D \w \W \s \S \b \B` in a field's `pattern` in the Database
Manager config, instead of rejecting them as an error, and document what
they mean, since Rust's `regex` is now the only engine that ever checks a
value.

## Context

The config overlay (`crates/fm-dbmanager/src/model/`) resolves each field's
`pattern`. `model/rule.rs::scan_pattern` checks that a pattern stays in the
common subset of Rust `regex` and JavaScript `RegExp` (`u` flag), because a
TypeScript mirror of validation was planned to run while the user typed.
That mirror is dropped: the UI is moving to a native Rust UI that calls
`fm_dbmanager::rules::validate` directly. The human decided to lift the
**shorthand-class** ban only.

Today, in `model/rule.rs`, `escape()` returns an error for `'d' | 'D' | 'w' | 'W' | 's' | 'S' | 'b' | 'B'`
with the hint `SHORTHAND_HINT`; the diagnostic is an **error**, the pattern
is dropped and the field becomes read-only. Tests:
`model/tests.rs::pattern_shorthand_rejected` and the shorthand rows of
`pattern_scanner_rejects_dialect_differences`. The reference
`docs/dbmanager-config.md` ("Pattern dialect", about line 337, and the
diagnostics table row about line 606) lists them as rejected.

### What to change

- `escape()`: `\d \D \w \W \s \S \b \B` are accepted, inside and outside a
  class (except `\b \B` inside a class, which Rust's `regex` rejects: leave
  that to the existing compile check, which already reports a pattern that
  does not compile). Remove `SHORTHAND_HINT` if unused.
- Everything else the scanner rejects or warns about stays exactly as it is
  (inline flags, look-around, `\p`/`\P`, `\A \z \Z`, back-references,
  nested classes, set operators, the punctuation rules, and the warning on
  an unescaped `.`). Do not loosen them.
- `docs/dbmanager-config.md`: move the shorthand classes out of "Rejected"
  and into the allowed list, with their Rust meaning stated plainly: they are
  **Unicode-aware** (`\d` matches any Unicode decimal digit, such as
  Arabic-Indic digits; `\w` any Unicode letter, digit or connector; `\s` any
  Unicode white space; `\b` a Unicode word boundary); write `[0-9]` for ASCII
  digits only. Update the opening sentence of "Pattern dialect": Rust's
  `regex` is the only engine; the remaining restrictions are kept so that
  existing configs keep their meaning. Update the diagnostics table row.
- The change only loosens: every config that was valid stays valid and
  means the same.

### Rules that bind this task

- `fm-dbmanager` has no UI dependency; fmt clean; clippy pedantic
  `-D warnings`; scoped `#[expect]` only.
- Tests use temp dirs if they touch files (these do not).
- If `apps/desktop/` and ts-rs still exist, this change touches no boundary
  type and needs no regeneration.

## Files

Edit: `crates/fm-dbmanager/src/model/{rule.rs,tests.rs}`,
`docs/dbmanager-config.md`.

Do not touch: other modules, `examples/dbmanager/validation-cases.json`
(unless a vector asserts the rejection; then update that vector and say so),
`docs/plans/`.

## Acceptance criteria

- A config with `pattern = '\d{4}'` resolves with no error and the field
  stays editable; `validate` accepts `"2024"` and `"٢٠٢٤"` (Arabic-Indic
  digits) and rejects `"20a4"`.
- Patterns with the other rejected constructs still give their errors.
- `docs/dbmanager-config.md` states the Unicode meaning of each shorthand.
- `cargo test -p fm-dbmanager` and the workspace commands pass.

## Tests proving completion

- `pattern_shorthand_allowed` (replaces `pattern_shorthand_rejected`: each
  former case resolves with zero errors and keeps its pattern, except
  `[\b]`-style cases that do not compile, which keep the compile error).
- `pattern_shorthand_is_unicode_aware` (`\d` accepts Arabic-Indic digits
  through `validate`).
- `pattern_scanner_rejects_dialect_differences` without its shorthand rows,
  all other rows unchanged.
- `pattern_dot_warns` unchanged.

## Out of scope

- Lifting any other restriction of the scanner.

## Risks

- None for existing configs (loosening only). A new config using `\d`
  accepts non-ASCII digits; the docs say so and show `[0-9]`.
