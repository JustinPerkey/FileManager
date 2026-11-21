# M15 — Allow the full Rust `regex` syntax in config patterns

Status: approved (2026-11-21; new that day: the human decided to lift every
old Rust/JavaScript pattern restriction, since only Rust validates now; the
TypeScript validation mirror is dropped)
Project: dbmanager   Depends on: M5 (landed)

## Goal

Accept any pattern that Rust's `regex` crate compiles (anchored) in a
field's `pattern` in the Database Manager config, removing the
"common subset" scanner and its warning, and document the full Rust syntax
as the pattern dialect.

## Context

The config overlay (`crates/fm-dbmanager/src/model/`) resolves each field's
`pattern`. `model/rule.rs` has `PatternScan` and `scan_pattern`, which keep
a pattern inside the common subset of Rust `regex` and JavaScript `RegExp`
(`u` flag), because a TypeScript mirror of validation was planned to run
while the user typed. That mirror is dropped: the UI is moving to a native
Rust UI that calls `fm_dbmanager::rules::validate` directly. The human
decided to lift **all** of those restrictions: the full Rust `regex` syntax
is allowed.

What exists:

- `model/fields.rs` (around line 322): for a text field, `scan_pattern`
  runs first; a `problem` is an **error** (`` `pattern` <problem> ``,
  pattern dropped, field read-only); otherwise the anchored form
  `^(?:<pattern>)$` must compile (`regex::Regex::new`), else the error
  "`pattern` does not compile: …"; then `scan.dot` adds the **warning** about
  an unescaped `.` and JavaScript.
- `model/rule.rs`: `PatternScan`, `SHORTHAND_HINT`, `differs`, `escape`,
  `quantifier_len`, `note`, `scan_pattern` (and `real_text`, unrelated:
  keep it).
- `config/validate.rs` (around line 241) and `rules/validate.rs` (around
  line 233) compile the same anchored form; they stay.
- Tests in `model/tests.rs`: `pattern_shorthand_rejected`,
  `pattern_dot_warns`, `pattern_scanner_rejects_dialect_differences` and
  any other test asserting a scanner error or the dot warning.
- `docs/dbmanager-config.md`: the "Pattern dialect" paragraphs (about lines
  337–370: the common subset, "Rejected, and why", the `.` warning), the
  sentence at about line 330, and the diagnostics-table rows at about lines
  605–607.

### What to change

- `model/fields.rs`: drop the scanner call and the dot warning. A text
  field's `pattern` is accepted when the anchored form compiles; otherwise
  the existing "does not compile" error applies, unchanged.
- **Anchoring stays sound:** without the scanner, a pattern such as
  `a)|(b` could compile only once wrapped (`^(?:a)|(b)$`) and lose its
  anchoring. Also require that the pattern compiles **on its own**
  (unanchored); if it does not, give the same "does not compile" error. Do
  this in `model/fields.rs` and in `config/validate.rs` if that module
  reports compile errors too (one shared helper, e.g. `rules::compile_anchored(pattern) -> Result<Regex, String>`,
  used by all three call sites).
- `model/rule.rs`: delete `PatternScan`, `SHORTHAND_HINT`, `differs`,
  `escape`, `quantifier_len`, `note` and `scan_pattern`; keep `real_text`.
  Update the module doc comment.
- `docs/dbmanager-config.md`: replace "Pattern dialect" with: the pattern is
  Rust `regex` syntax (link `https://docs.rs/regex/latest/regex/#syntax`),
  anchored as `^(?:pattern)$`, and the field's whole value must match. Say
  plainly that the shorthand classes are **Unicode-aware** (`\d` matches any
  Unicode decimal digit, for example Arabic-Indic digits; `\w` and `\s` are
  Unicode; `\b` is a Unicode word boundary) and that `[0-9]`, `[A-Za-z0-9_]`
  and `[ \t\r\n]` give the ASCII sets; that `.` matches any character except
  `\n`; that look-around and back-references are not supported by Rust's
  `regex` and therefore do not compile. Replace the three diagnostics rows
  with one: "`pattern` does not compile (alone or anchored) | error |
  pattern dropped; field read-only". Remove every mention of JavaScript and
  of the common subset from the document.
- `examples/dbmanager/example.toml` and `validation-cases.json` need no
  change unless a case asserts a scanner error; if one does, update it and
  say so.
- This only loosens (apart from the anchoring guard, which rejects only
  patterns that never anchored correctly): every config that was valid stays
  valid and means the same.

### Rules that bind this task

- `fm-dbmanager` has no UI dependency; fmt clean; clippy pedantic
  `-D warnings`; scoped `#[expect]` only.
- If `apps/desktop/` and ts-rs still exist, nothing here changes a boundary
  type; no regeneration.

## Files

Edit: `crates/fm-dbmanager/src/model/{rule.rs,fields.rs,tests.rs}`,
`crates/fm-dbmanager/src/config/validate.rs` and
`crates/fm-dbmanager/src/rules/{mod.rs,validate.rs}` (only for the shared
compile helper), `docs/dbmanager-config.md`.

Do not touch: other modules, `docs/plans/`.

## Acceptance criteria

- Configs with `pattern = '\d{4}'`, `'(?i)abc'`, `'\p{Greek}+'`,
  `'[[:alpha:]]+'`, `'[a-z&&[^aeiou]]+'`, `'a.b'`, `'\Aab\z'`,
  `'[]a]'` (if Rust compiles it) resolve with **no** diagnostic, and the
  field stays editable.
- `(?=a)`, `\1` and `a)|(b` give the "does not compile" error.
- `validate` with `\d{4}` accepts `"2024"` and `"٢٠٢٤"` and rejects `"20a4"`.
- `docs/dbmanager-config.md` contains no "JavaScript" and no "common
  subset"; it documents the Unicode meaning of the shorthand classes.
- `cargo test -p fm-dbmanager` and the workspace commands pass.

## Tests proving completion

In `model/tests.rs`:

- `pattern_full_rust_syntax_allowed` (one case per construct above).
- `pattern_shorthand_is_unicode_aware` (through `validate`).
- `pattern_that_does_not_compile_is_error` (look-around, back-reference).
- `pattern_must_compile_alone` (`a)|(b`).
- `pattern_dot_has_no_warning`.
- The removed tests (`pattern_shorthand_rejected`, `pattern_dot_warns`,
  `pattern_scanner_rejects_dialect_differences`) are listed in the PR.

## Out of scope

- Any other config rule; the UI.

## Risks

- A new config using `\d` accepts non-ASCII digits; the docs say so and
  show `[0-9]`.
- The "compiles alone" guard rejects a pattern that compiled before only
  because of the wrapping; such a pattern never matched as its author meant.
  Name any example config affected (none is expected).
