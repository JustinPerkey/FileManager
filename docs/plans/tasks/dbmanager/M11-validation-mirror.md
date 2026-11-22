# M11 — TypeScript validation mirror

Status: **dropped (2026-11-21):** the UI moves to GPUI and validates with `fm_dbmanager::rules::validate` directly. Do not run this plan. Previous status: awaiting approval (renumbered from M10 and revised 2026-11-16:
durations, link fields)
Project: dbmanager   Depends on: M4 (landed). Independent of M5–M10; may run
in parallel with them.

## Goal

Add `apps/desktop/src/lib/dbmanagerValidation.ts`, a TypeScript mirror of the
Rust field validation, so the object form and many-to-many link fields can
give inline feedback, and prove it agrees with Rust by running the same shared
vectors.

## Context

The Database Manager (`dbmanager`) validates every field change in Rust
before writing (`crates/fm-dbmanager/src/rules/validate.rs`, `validate(rule,
input) -> Result<DbValue, Vec<FieldIssue>>`). That is authoritative: the save
result always wins. The UI also wants feedback while the user types, so the
pure part of the rules is mirrored here. The rule *data* is not duplicated:
`FieldRule` (with `unit`), `DbValue` (with the `duration` variant),
`FieldIssue`, `FieldIssueCode`, `FieldKind` and `DurationUnit` are generated
by ts-rs into `apps/desktop/src/lib/generated/`. Link fields (the extra columns
of a many-to-many junction) use the same `FieldRule`, so the same function
validates them. Only the logic is
mirrored, and **parity is proven by data**:
`examples/dbmanager/validation-cases.json` already passes in Rust
(`shared_vectors_pass`) and must pass here unchanged.

The semantics are in `docs/dbmanager-config.md` ("Fields", "Kinds",
"Durations") and, exactly, in the Rust source above
(`crates/fm-dbmanager/src/rules/validate.rs` and `duration.rs`). Restated for
this task (Rust is the reference if anything here is unclear):

1. **Null** (`{type:"null"}`): ok (value `null`) if `rule.nullable`, else
   `required`.
2. **Shape**: integer kind takes `integer`; real takes `real`; text takes
   `text`; bool takes `integer`; duration takes `duration`; anything else, and
   any input for `blob` or `any`, is `wrongType` (stop).
3. **Integer**: `^[+-]?[0-9]+$`, within the signed 64-bit range (use
   `BigInt`) — else `notInteger` (stop). Normalised: canonical decimal (no
   `+`, no leading zeros, `-0` → `0`). Then `belowMin`/`aboveMax`
   (`BigInt(rule.min)` …), then `notAllowed` (compare canonical text with
   the `allowed` values).
4. **Real**: `^[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)([eE][+-]?[0-9]+)?$` and
   `Number.isFinite(Number(s))` — else `notReal` (stop). Normalised: the input
   string unchanged. Then `belowMin`/`aboveMax` (as numbers).
5. **Bool**: canonical integer `0` or `1`, else `notBool`. Normalised
   `"0"`/`"1"`.
5a. **Duration** (unit = `rule.unit`; ms per unit: milliseconds 1, seconds
   1 000, minutes 60 000, hours 3 600 000, days 86 400 000):
   - Grammar: optional `-`, then either a bare integer (a count of the unit) or
     one or more `<digits><suffix>` components with suffixes in strictly
     decreasing order from `d`, `h`, `m`, `s`, `ms`, separated by at most one
     ASCII space; lowercase; no leading/trailing space, no decimals. Else
     `notDuration` (stop). Beware `m` vs `ms` when tokenising.
   - Total milliseconds as `BigInt`; not a whole multiple of the unit →
     `durationPrecision` with `limit` = the unit name (e.g. `"seconds"`)
     (stop); count outside the signed 64-bit range → `notDuration` (stop).
   - Normalised: `{type:"duration", value: canonical}` where canonical is the
     sign, then the non-zero components from `d` down to the unit's suffix
     (`ms`, `s`, `m`, `h`, `d`), no spaces; zero is `0` + the unit's suffix.
   - Then `belowMin`/`aboveMax`: parse `rule.min`/`rule.max` (canonical
     duration text) the same way and compare counts; `limit` is the bound
     text.
6. **Text**: `rule.emptyIsNull && rule.nullable && s === ""` → ok `null`.
   Else collect `tooShort`, `tooLong` (length in code points: `[...s].length`,
   which equals Rust's `chars().count()`), `patternMismatch` (`new
   RegExp("^(?:" + p + ")$", "u")`), `notAllowed` (exact match).
7. Issues are returned in this code order: `required`, `wrongType`,
   `notInteger`, `notReal`, `notBool`, `notDuration`, `durationPrecision`,
   `belowMin`, `aboveMax`, `tooShort`,
   `tooLong`, `patternMismatch`, `notAllowed`. `limit` is the violated bound
   as text (`rule.min`, `rule.max`, `String(rule.minLength)`, the pattern
   source), else omitted/null exactly as the generated `FieldIssue` type
   says. `message` is a short English sentence (not compared with Rust).
8. The database-dependent codes (`unique`, `referenceMissing`,
   `notEditable`, `unknownField`) are **never** produced here.
9. The model rejects every construct known to differ between Rust `regex`
   and JS `RegExp` (`u` flag): shorthand classes, nested classes, POSIX
   classes, set operators, inline flags and look-around, `\A`/`\z`/`\Z`,
   `\p`/`\P`, backreferences, and escapes outside
   `\\ \^ \$ \. \* \+ \? \( \) \[ \] \{ \} \| \/ \-` and
   `\t \n \r \f \v \xHH`. So in practice every pattern a `FieldRule`
   carries compiles in JS.
10. If JavaScript still cannot compile a pattern that Rust accepted (dialect
   difference), skip the pattern check for that field (the save result
   decides) and do not throw. Cache compiled patterns.

**Behaviours pinned by the vectors** (from the M4 review). The TS mirror must
reproduce them exactly:

- for bool:
  - an `integer`-variant input whose text is not an integer, e.g.
    `{type:"integer", value:"yes"}`, is **`notBool`**, not `notInteger` (as
    the landed vector pins it);
  - any integer other than canonical `0`/`1` is `notBool`;
  - `"+1"` and `"01"` normalise to `"1"`;
  - a `text`-variant input is `wrongType` (shape);
- a duration rule whose `unit` is `null` → `notDuration`;
- `validate` ignores `min`, `max` and `allowed` for bool, and `allowed` for
  real and duration;
- a `min`/`max` that does not parse for the kind is skipped. The model
  milestone guarantees canonical bounds, so the mirror may assume them, but
  must not throw if one is malformed;
- patterns never contain shorthand classes (`\d \w \s \b` and their
  negations; the model rejects them). The vectors include non-ASCII digits
  and spaces against explicit classes (`[0-9]`, `[ \t\r\n]`), which must
  give the same result as Rust.


Rules that bind this task:

- The implementer owns `apps/desktop/src/lib/`; do not touch the rest of
  `apps/desktop/src/` (views belong to the ui-implementer).
- Use only the generated types; never hand-edit `lib/generated/`; never
  redeclare a generated type.
- No change to Rust, to the vectors' meaning, or to `generated/`. If a vector
  cannot pass in TypeScript because Rust's behaviour is not reproducible,
  stop and report it (the fix belongs in Rust and the vectors).
- `npm run typecheck && npm run lint && npm run test` pass.

### API

```ts
export type ValidationResult =
  | { ok: true; value: DbValue }
  | { ok: false; issues: FieldIssue[] };

/** Mirrors fm_dbmanager::rules::validate. Inline feedback only; the save result is authoritative. */
export function validateField(rule: FieldRule, input: DbValue): ValidationResult;

/**
 * Columns of fields with requiredOnCreate that are absent from `supplied`.
 * Works for an object's fields on create and for M2mView.linkFields when adding
 * a link (requiredOnCreate there means "required when adding the link").
 */
export function missingRequiredOnCreate(fields: FieldView[], supplied: ReadonlySet<string>): string[];

/** Parses duration text in `unit` and returns the canonical text, or null if invalid. */
export function canonicalDuration(text: string, unit: DurationUnit): string | null;
```

`FieldView` is generated by the model milestone; if it is not yet in
`lib/generated/` when you run, implement `missingRequiredOnCreate` against a
local structural type `{ column: string; requiredOnCreate: boolean }[]` and
note it in your hand-off.

### Test

`apps/desktop/src/lib/dbmanagerValidation.test.ts`:

- Load the vectors with a Vite raw import,
  `import raw from "../../../../examples/dbmanager/validation-cases.json?raw"`,
  and `JSON.parse` it (the `?raw` suffix is typed by `vite/client`, already in
  `tsconfig.json`). Adjust the relative path if needed.
- `it.each` over every case: `ok` cases must return `{ ok: true }` with an
  equal `value`; failing cases must return the same codes in the same order,
  and the same `limit`s when the case lists them.
- Assert the case count is at least 80, and at least 20 cases have a
  `duration` rule, so an empty or truncated file cannot pass.
- Extra tests: never returns database-dependent codes; a pattern JS cannot
  compile (for example one using Rust-only syntax like `(?i)abc` — check it
  really fails in JS) is skipped without throwing;
  `missingRequiredOnCreate` for object fields and link fields;
  `canonicalDuration` examples (`5400` seconds → `"1h30m"`, `"1h 30m"` →
  `"1h30m"`, `"1500ms"` in seconds → `null`, `"2d"` in hours → `"2d"`).

## Files

Create `apps/desktop/src/lib/dbmanagerValidation.ts` and
`apps/desktop/src/lib/dbmanagerValidation.test.ts`. Edit `CLAUDE.md` layout
(`src/lib/dbmanagerValidation.ts`: the TS mirror of the dbmanager field
rules, kept in parity by `examples/dbmanager/validation-cases.json`).

Nothing else.

## Acceptance criteria

- Every vector passes in vitest, unchanged.
- `npm run typecheck && npm run lint && npm run test` pass.
- `cargo test --workspace` still passes (nothing in Rust changed).

## Tests proving completion

`dbmanagerValidation.test.ts`: `shared vectors` (each case),
`vector file is complete`, `never returns database-only codes`,
`skips patterns JS cannot compile`, `missingRequiredOnCreate`,
`canonicalDuration`.

Run: `npm run typecheck && npm run lint && npm run test`.

## Out of scope

- Any UI, wording, or presentation of issues (ui-implementer).
- Changing Rust validation or the vectors.

## Risks

- `BigInt("+5")` and leading-zero handling: test them; normalise explicitly.
- Duration tokenising: `"5ms"` must not be read as `5m` + `s`; write an
  explicit tokenizer rather than one clever regex if in doubt.
- Importing a file outside the Vite root works in vitest; if it does not in
  this setup, read the file with `node:fs` in the test instead and report the
  change.
