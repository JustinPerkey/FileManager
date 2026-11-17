# M3 — Config format: parsing, structural validation, reference doc, examples

Status: implemented from an outdated draft; the reviewer requested changes.
**Run "Review round 1" (end of this plan)** on this task plan; the sections
above are the approved scope and already include its decisions. (Approved
2026-11-17; revised 2026-11-16: editable defaults, durations,
link fields; then the human's final answers: the `pickable` table mode, one
fixed app-wide config file that reloads automatically, a starter file)
Project: dbmanager   Depends on: M1 (landed). Can run in parallel with M2.

## Goal

Parse the Database Manager's TOML config into a typed `Config` with
line/column diagnostics, write the config reference `docs/dbmanager-config.md`
(from now on the format's only home), and add the example schema and config
under `examples/dbmanager/`.

## Context

The Database Manager (`dbmanager`) opens a user's SQLite file. Table
structure, column types and foreign keys come from the database. M2 builds
that introspection as `crates/fm-dbmanager/src/schema/`. The TOML config is
an **overlay**. It says which tables, fields and many-to-many relations are
editable, read-only, hidden or create-only. It sets field bounds and
declares labels, durations and many-to-many relations. **Anything it does
not mention is editable** (the human's decision), except the columns that
can never be edited.

This milestone covers the part that needs **no database**: TOML syntax,
keys, value types, and the rules that can be checked from the config alone.
Matching names against the schema, kind compatibility and the other
schema-dependent rules below are implemented in M5. The reference doc
written here still documents them **completely**, because later task plans
point at the doc.

Builds on M1: the crate `crates/fm-dbmanager` (`src/lib.rs`, `db.rs`,
`state.rs`). Crate rules:

- no tauri;
- `[lints] workspace = true`;
- fmt and clippy pedantic clean;
- scoped `#[expect]` only;
- filesystem tests in temp dirs.

Technique: `crates/fm-tarpack/src/manifest/` (`raw.rs`, `validate.rs`)
already parses TOML with `toml::de::DeTable` and turns spans into 1-based
line/column diagnostics. Read it for the technique, and reproduce what you
need inside `fm-dbmanager`.

- **Do not depend on `fm-tarpack`.** Tools never import each other.
- Use the same `toml` version and features as `fm-tarpack`'s `Cargo.toml`.
- Add the `regex` crate for `pattern`.

### Boundary types introduced here

Config diagnostics reach the UI later. Define them with `#[derive(Clone,
Debug, PartialEq, Eq, Serialize, ts_rs::TS)]` and `#[serde(rename_all =
"camelCase")]`, and add `ts-rs = { workspace = true }`:

```rust
pub enum DiagnosticSeverity { Error, Warning }        // serde: "error" | "warning"
pub struct ConfigDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub line: Option<u32>,    // 1-based, config diagnostics only
    pub col: Option<u32>,     // 1-based
    pub table: Option<String>,
    pub column: Option<String>,
}
```

`TableMode`, `FieldMode`, `RelationMode` and `DurationUnit` (below) derive
`ts_rs::TS` too. The modes serialize kebab-case (`"editable" | "read-only" |
"hidden" | "create-only"`) and `DurationUnit` lowercase (`"milliseconds" |
"seconds" | "minutes" | "hours" | "days"`).

The names must not collide with files already in
`apps/desktop/src/lib/generated/`. For example, `Diagnostic` and `Severity`
belong to the Tar Packager. Exporting is M4's job, so do not edit the shell
in this milestone.

### API

```rust
pub fn parse(text: &str) -> ConfigReport;   // src/config/mod.rs
pub struct ConfigReport {
    pub config: Config,
    pub diagnostics: Vec<ConfigDiagnostic>,
    /// Some(short reason) iff `config` is `Config::fail_closed()` because the whole
    /// config could not be used ("syntax error at line 4", "unsupported version 2").
    /// None for every other config, including an empty one. The UI shows this
    /// reason; it must never infer fail-closed from diagnostic text.
    pub fail_closed: Option<String>,
}
```

`Config` always exists. Two constructors matter:

- `Config::default()`: no entries, built-in defaults (all `editable`). This
  is what an empty or entry-less config means.
- `Config::fail_closed()`: no entries, all three default modes `read-only`.
  It is used when the config's intent cannot be known. That covers a syntax
  error, a `version` other than 1, and, from M6 on, a config file that cannot
  be read. Without this, a typo would make everything editable, including
  tables the user meant to hide. `ConfigReport.fail_closed` carries the
  reason whenever it is used.

Diagnostics are sorted by (line, col). A leading UTF-8 BOM is accepted.

The model is below. The names are indicative. Every entry keeps the 1-based
line/col of its header key, every bound keeps its key's line/col, and every
optional setting records whether it was **set explicitly**, because M5
reports explicit and inherited settings differently.

```rust
pub struct Config { pub defaults: Defaults, pub tables: Vec<TableEntry>, pub many_to_many: Vec<M2mEntry> }
pub struct Defaults { pub table_mode: TableMode, pub field_mode: FieldMode, pub relation_mode: RelationMode }
// Defaults::default(): all three Editable
pub enum TableMode { Editable, ReadOnly, Hidden, Pickable }   // serde "pickable"; never valid in [defaults]
pub enum FieldMode { Editable, ReadOnly, Hidden, CreateOnly }
pub enum RelationMode { Editable, ReadOnly, Hidden }
pub enum DurationUnit { Milliseconds, Seconds, Minutes, Hours, Days }
pub enum TableEntry { Valid(TableConfig), Rejected { name, at, hidden: bool, fields: Vec<FieldEntry> } }
pub enum FieldEntry { Valid(FieldConfig), Rejected { name, at, hidden: bool } }
pub enum M2mEntry  { Valid(M2mConfig),   Rejected { name, at } }
pub struct LabelTemplate { pub parts: Vec<LabelPart> }  // Literal(String) | Column(String)
pub enum Bound { Int(i64), Float(f64), Duration(String) }   // Duration: the human text as written
pub enum AllowedValues { Integers(Vec<(i64, Option<String>)>), Texts(Vec<(String, Option<String>)>) }
```

`Rejected` is the **fail-closed** rule. An entry with any error is dropped,
and its subject becomes **read-only**, not the editable default. It becomes
hidden instead if the entry's own `mode` key was valid and said `hidden`.

A rejected **table** entry still keeps its parsed field entries in `fields`,
whether they are valid or rejected. The model uses them only to keep hidden
fields hidden. Otherwise a table-level typo would reveal a column the user
had marked `mode = "hidden"`. Every other effect of those field entries is
dropped, and the table's fields are read-only.

Durations: this milestone checks that a `Bound::Duration` string matches the
duration grammar (see "Durations" below; write a small parser in
`src/config/`, or a shared one in `src/rules/duration.rs` if M4 has landed).
Checking that it is a whole number of the field's unit is M5's job, because
it needs the resolved unit.

## Reference content (write this into `docs/dbmanager-config.md`)

Write the doc for a user who edits the file by hand:

1. purpose;
2. a complete example (the one in `examples/dbmanager/example.toml`);
3. one section per topic below;
4. the diagnostics table.

Everything below is normative.

### File

There is **one config for the whole app**, at a fixed location the user does
not choose:

- Windows: `%ProgramData%\FileManager\dbmanager.toml` (normally
  `C:\ProgramData\FileManager\dbmanager.toml`);
- other platforms (development builds only): `/etc/filemanager/dbmanager.toml`.

The app only reads it; it never writes it, except that developer mode
(Ctrl+Alt+Shift+D) offers **Edit config**, which opens the file in the system
editor and, if the file does not exist yet, first creates a commented starter
file there (never replacing an existing file). Saved edits are picked up
**automatically**: the app watches the file and reloads it, keeping unsaved
edits in the form and re-checking them against the new rules. A save that was
prepared against the old config is refused once and can be retried.

The same config applies to every database the app opens. Table and field names
that a database does not have are warnings, not errors, so one file can
describe several databases.

UTF-8 TOML, optional BOM. Top-level keys: `version`, `defaults`, `tables`,
`many_to_many`.

Any other top-level key (for example the typo `[table.secrets]`) is an error,
is ignored, and **narrows all three defaults to `read-only`**, as a
wrong-typed `defaults`, `tables` or `many_to_many` does. A misspelt section
could otherwise leave the tables it meant to restrict editable. Entries that
parsed correctly still apply.

- `version = 1`. If it is missing: a warning, read as 1. Any other value: an
  error. The whole config is ignored and **everything is read-only** until it
  is fixed.
- A TOML syntax error is an error at its position. The whole config is
  ignored and **everything is read-only** until it is fixed.
- A config file that cannot be read (I/O error, not UTF-8): everything is
  read-only.
- No config file at all: the built-in defaults apply, so everything is
  editable except forced read-only columns, and the app says that no config
  is in use.

### Defaults

```toml
[defaults]
table_mode = "editable"      # editable | read-only | hidden
field_mode = "editable"      # editable | read-only | hidden
relation_mode = "editable"   # editable | read-only | hidden
```

All three default to `editable`. They apply to every table, field and
many-to-many relation the config does not mention. To make a database
read-only unless stated otherwise, write `table_mode = "read-only"`.
`create-only` is not a valid default. An invalid `[defaults]` (unknown key or
bad value) is an error, and all three defaults become `read-only` (fail
closed). The entries that are valid still apply.

Whatever the defaults say, some columns are never editable (see "Forced
read-only"), and an entry with an error makes its subject read-only.

### Tables — `[tables.<name>]`

`<name>` is the SQL table name. Quote it in TOML if needed
(`[tables."order items"]`). Keys:

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `mode` | `"editable" \| "read-only" \| "hidden" \| "pickable"` | `defaults.table_mode` | `hidden`: never listed or linked. `read-only`: listed and browsable, nothing changes. `pickable`: **not listed**, but its objects are offered in pickers for foreign keys into it, appear as links (following one opens the object read-only), and appear as the other side of many-to-many checklists; nothing in it changes (a lookup table). `editable`: fields per their modes. `pickable` is not valid in `[defaults]`. For a junction used as many-to-many, see "Many-to-many". |
| `display_name` | string | the SQL name | shown for the table |
| `description` | string | none | shown with the table |
| `object_name` | string | none | the singular noun for one object, lower case as it reads mid-sentence (`"author"`), so the app can say "New author" instead of "New object in Authors" |
| `label` | template | see Labels | an object's title |
| `order_by` | array | label columns, then key | list order; elements `"col"` or `{ column = "col", descending = true }` |
| `field_order` | array of strings | column order | these fields first, the rest in column order |
| `allow_create` | bool | `true` | "New" is offered (editable tables only) |
| `allow_delete` | bool | `false` | "Delete" is offered (editable tables only). Off by default because deleting destroys data and can cascade into other tables. |
| `fields` | table of field entries | none | `[tables.<name>.fields.<column>]` |

### Fields — `[tables.<table>.fields.<column>]`

The same entries configure a junction's **link fields** (see
"Many-to-many").

| Key | Applies to kind | Meaning |
| --- | --- | --- |
| `mode` | all | `editable`, `read-only`, `hidden`, or `create-only` (set when creating the object, or when adding the link for a link field; read-only after). Default `defaults.field_mode`. |
| `display_name`, `help` | all | label and help text |
| `type` | none | `"integer" \| "real" \| "text" \| "bool" \| "duration"`: override the inferred kind (see Kinds) |
| `unit` | duration | `"milliseconds" \| "seconds" \| "minutes" \| "hours" \| "days"`, default `"seconds"`: what one stored integer counts |
| `nullable` | all | `false` forbids NULL. `true` is allowed only if the column allows NULL. Default: the column's `NOT NULL`. |
| `min`, `max` | integer, real, duration | Inclusive. Integer: TOML integers. Real: integers or floats. Duration: a duration string (`"30s"`, `"1h30m"`) or an integer count of the unit. |
| `min_length`, `max_length` | text | Inclusive, counted in Unicode scalar values (characters), non-negative. |
| `pattern` | text | A regular expression the **whole** value must match. It is anchored as `^(?:pattern)$`. |
| `allowed` | integer, text | The only permitted values: `["a", "b"]`, `[1, 2]`, or `[{ value = "a", label = "Alpha" }, …]`. |
| `empty_is_null` | text, nullable fields | An empty input is stored as NULL. Default `true` when the field is nullable. |
| `multiline` | text | Display hint: a multi-line editor. |

Checked from the config alone. Each of these is an error that rejects the
field entry:

- an unknown key, a wrong value type, or an unknown `mode`/`type`/`unit`
  string;
- `unit` without `type = "duration"`;
- `min > max` when both have the same form (two integers, two numbers, or two
  durations), `min_length > max_length`, a negative length, or a length above
  `u32::MAX`;
- a duration bound string that does not match the duration grammar;
- a `pattern` that Rust's `regex` crate cannot compile when anchored;
- `allowed` that is empty, mixes strings and integers, contains a duplicate
  value, has an inline table with keys other than `value`/`label`, is
  missing a `value`, or has an empty (all-whitespace) `label`;
- an empty (all-whitespace) `display_name`, `help`, `description` or
  `object_name`.

Pattern dialect: Rust's `regex` is authoritative on save, while the UI checks
the same pattern with JavaScript `RegExp` (`u` flag) as the user types. Stay
in their common subset: character classes, `\d \w \s`, quantifiers, groups,
alternation. No look-around, no back-references, no inline flags.

### Kinds

A field's kind decides its editor and its bounds. The kind is inferred from
the declared column type, upper-cased and trimmed with any `(...)` suffix
removed, by these rules in order:

1. `BOOLEAN` or `BOOL` → **bool**
2. `ANY`, or empty → **any**
3. contains `INT` → **integer**
4. contains `CHAR`, `CLOB` or `TEXT` → **text**
5. contains `BLOB` → **blob**
6. contains `REAL`, `FLOA` or `DOUB` → **real**
7. otherwise (NUMERIC affinity) → **real**

**duration** is never inferred. Only `type = "duration"` gives it.

`type` overrides the kind only where the column's affinity stores it
faithfully:

| `type` | allowed on |
| --- | --- |
| `integer` | INTEGER or NUMERIC affinity, untyped/`ANY` |
| `real` | REAL or NUMERIC affinity, untyped/`ANY` |
| `text` | TEXT or NUMERIC affinity, untyped/`ANY` |
| `bool` | INTEGER or NUMERIC affinity, untyped/`ANY` |
| `duration` | INTEGER or NUMERIC affinity, untyped/`ANY` |

In STRICT tables: `INT`/`INTEGER` take integer, bool or duration. `REAL`
takes real, `TEXT` takes text, `ANY` takes any of these, and `BLOB` takes
none.

Booleans are stored as INTEGER `0` (false) and `1` (true). A stored `NULL`
is "not set" (nullable fields only). Any other stored value is shown as a
problem.

`blob` and `any` fields are never editable. A blob is shown as its size, and
`any` becomes editable only with a `type`.

### Durations

A duration field stores an **INTEGER count of its `unit`**. A day is exactly
24 hours.

- **Written form:** an optional `-`, then either:
  - a bare integer, which is a count of the field's unit (`90` with unit
    seconds is 90 seconds); or
  - one or more components `<digits><suffix>`, with suffixes in strictly
    decreasing order from `d`, `h`, `m`, `s`, `ms`. Components may be
    separated by single spaces.

  Examples: `1h30m`, `1h 30m`, `2d`, `250ms`. Suffixes are lowercase. Nothing
  else is accepted: no leading or trailing spaces, no repeated or out-of-order
  suffixes, no decimals.
- **Precision:** the value must be a whole number of the unit. With unit
  `minutes`, `90s` is rejected and `2m` is accepted.
- **Canonical (shown) form:** the sign, then the non-zero components from
  `d` down to the field's unit, with no spaces. For example, 5400 seconds is
  `1h30m`. Zero is written `0` followed by the unit's suffix, such as `0s`.
- `min`/`max` use the written form or an integer count of the unit, and must
  be whole numbers of the unit.
- `allowed`, `pattern` and lengths do not apply.
- Search skips duration columns.

### Forced read-only

These are always read-only, whatever the config says:

- a rowid alias (`INTEGER PRIMARY KEY`, assigned by SQLite);
- generated columns;
- `blob` columns, and untyped/`any` columns without `type`;
- foreign keys of more than one column;
- foreign keys into a hidden table.

The primary-key columns of an existing object are never editable either. An
inherited `editable` on one becomes `create-only`, so it can be set when
creating.

Asking explicitly for `editable` (or `create-only`, except on a primary-key
column) on one of these is an error. Inheriting it from `[defaults]` silently
narrows it.

### Labels

`label = "{last_name}, {first_name}"`: `{column}` inserts a column's value,
and `{{` and `}}` are literal braces. Placeholders must name visible (not
hidden) columns of the table. Values render as follows:

- `NULL` renders as empty;
- integers and reals render as their decimal text;
- bools render as `true`/`false`;
- durations render in canonical form.

When `label` is absent, the default is `{c}`, where `c` is the first
non-hidden **text** column named `name`, `title` or `label`
(case-insensitive, in that order). If there is none, it is `"<display name>
#<key>"`, where `<key>` is the rowid or the primary-key values joined by
`, `. A label that renders empty also falls back to `"<display name> #<key>"`.

Search in a table's object list matches the label's non-duration columns
(substring, case-insensitive for ASCII). A table whose label has no such
columns is not searchable.

Checked here: an unbalanced `{` or `}`, or an empty `{}`, is an error, and
the table entry is rejected.

### Foreign keys and reverse relations

Foreign keys come from the database.

- A single-column FK field shows the referenced object's label as a link.
  Editing it picks from the referenced table's objects.
- A FK whose target table or column does not exist is a warning, and the
  column becomes a plain field.
- Bounds (`min`, `max`, lengths, `pattern`, `allowed`) and `type`/`unit` do
  not apply to a FK field. Setting one is an error, and the field becomes
  read-only. `nullable`, `mode`, `display_name` and `help` are fine.
- Every object also lists the objects that reference it ("referenced by"),
  except through junctions used as many-to-many.

### Many-to-many — `[many_to_many.<junction table>]`

**Detected** junctions are many-to-many without any config. A table is
detected as a junction when:

- it has exactly two single-column foreign keys to two **different** tables;
- the two columns together are its primary key or a unique index;
- it has no other column, except an optional `INTEGER PRIMARY KEY`.

**Declared** junctions are needed when the junction has extra columns, or
more than two foreign keys. Declaring an entry makes the table many-to-many
when:

- its `columns` (or its only two FKs) are single-column FKs to two different
  tables; and
- the pair is the primary key or a unique index.

Its other columns become the relation's **link fields**. Each link (one
junction row) carries their values.

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `enabled` | bool | `true` | `false`: treat the junction as an ordinary table |
| `mode` | `"editable" \| "read-only" \| "hidden"` | `defaults.relation_mode` | `hidden`: no checklist on either side. `editable`: links can be added, edited and removed. |
| `columns` | `[a, b]` | the two FKs | choose the two FK columns of a wider junction |
| `headings` | table: side table name → string | the other side's display name | the checklist heading shown on that side's objects |

**Link fields** are configured with ordinary field entries on the junction
table: `[tables.<junction>.fields.<column>]`, with the same modes, kinds,
durations and bounds as any field.

- `create-only` link fields are set when the link is added and are read-only
  after.
- Hidden link fields are never shown, and new links give them their column
  default.
- Link fields that are foreign keys are links with a picker, like any FK
  field.

The junction table itself:

- A junction used as many-to-many is **not listed** as a table of its own,
  unless its `[tables.<junction>]` entry sets a `mode`.
- For such a junction, `mode` only controls that listing (`hidden` = not
  listed). The links and link fields are governed by the relation's `mode`
  and the field entries.
- To hide the links themselves, set the relation's `mode = "hidden"`.

Where the checklist appears:

- It shows on a side only if the other side's table is not hidden.
- It is editable on a side only if the relation is `editable` **and** that
  side's table is `editable`.

If a `NOT NULL` junction column without a default can be supplied by no
editable or create-only link field, links cannot be added and the relation
is read-only. This is an error when the relation's `mode` was set
explicitly, and a warning when it was inherited.

Self-referential junctions (both foreign keys to the same table) are not
many-to-many in v1. Declaring one is a warning.

Checked here: an error rejects the entry, and the relation stays as detected,
read-only. The errors are:

- `columns` that is not exactly two distinct strings;
- a `headings` value that is not a non-empty string;
- unknown keys.

### Diagnostics

Errors never make anything more editable than its entry would have been. A
rejected entry leaves its subject read-only (or hidden, if it validly asked
for hidden). The database always opens and can be browsed.

| Situation | Severity | Effect |
| --- | --- | --- |
| TOML syntax error, `version` ≠ 1 | error | whole config ignored; everything read-only (fail closed) |
| invalid `[defaults]` | error | all defaults `read-only` |
| missing `version` | warning | read as 1 |
| unknown top-level key, or a wrong-typed `defaults`/`tables`/`many_to_many` | error | ignored; all defaults narrowed to `read-only`; valid entries still apply |
| unknown key, wrong type, invalid value (any rule above) | error | that table/field/relation entry rejected: its subject is read-only; a rejected table keeps its fields' `hidden` |
| unknown table, field, or junction | warning | entry ignored |
| names a view, virtual, shadow or internal table | warning | entry ignored |
| bound, `type` or `unit` not valid for the field's kind or affinity, or set on a foreign-key field | error | field read-only |
| duration bound not a whole number of the unit | error | field read-only |
| `nullable = true` on a `NOT NULL` column | error | field read-only |
| explicit `editable`/`create-only` on a forced-read-only column | error | field read-only |
| field explicitly `editable`/`create-only` in a read-only or pickable table | warning | stays read-only |
| `allow_create`/`allow_delete` set explicitly on a non-editable (read-only, pickable, hidden) table | warning | ignored |
| `table_mode = "pickable"` in `[defaults]` | error | `[defaults]` invalid (all defaults read-only) |
| `mode = "pickable"` on a junction used as many-to-many | warning | treated as not listed |
| a `NOT NULL` column that is neither `editable` nor `create-only`, has no default and is not a rowid alias, while create is on | error if `allow_create` was explicit, warning if inherited | create disabled |
| a junction `NOT NULL` column without default that no editable/create-only link field supplies, while the relation is editable | error if the relation `mode` was explicit, warning if inherited | relation read-only |
| label/order_by/field_order names an unknown or hidden column | error | that key ignored, default used |
| FK to a missing table/column | warning | plain field |
| self-referential junction declared | warning | not many-to-many |
| table with no usable identity | warning | table skipped |

Config diagnostics carry line and column. Database diagnostics carry the
table and column.

## Examples (`examples/dbmanager/`)

`schema.sql` holds DDL plus a few seed rows per table. It must run with
`execute_batch` on an empty database:

```sql
PRAGMA foreign_keys = ON;
CREATE TABLE publisher (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, country TEXT);
CREATE TABLE author (id INTEGER PRIMARY KEY, first_name TEXT NOT NULL, last_name TEXT NOT NULL,
                     born INTEGER, internal_note TEXT);
CREATE TABLE book (
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  isbn TEXT NOT NULL UNIQUE,
  author_id INTEGER NOT NULL REFERENCES author(id) ON DELETE CASCADE,
  publisher_id INTEGER REFERENCES publisher(id) ON DELETE RESTRICT,
  format TEXT NOT NULL DEFAULT 'paperback',
  pages INTEGER,
  price REAL,
  in_print BOOLEAN NOT NULL DEFAULT 1,
  audio_length INTEGER,
  summary TEXT,
  revision INTEGER NOT NULL DEFAULT 1);
CREATE TABLE genre (code TEXT PRIMARY KEY, name TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE book_genre (
  book_id INTEGER NOT NULL REFERENCES book(id) ON DELETE CASCADE,
  genre_code TEXT NOT NULL REFERENCES genre(code) ON DELETE CASCADE,
  PRIMARY KEY (book_id, genre_code)) WITHOUT ROWID;
CREATE TABLE contribution (
  book_id INTEGER NOT NULL REFERENCES book(id) ON DELETE CASCADE,
  author_id INTEGER NOT NULL REFERENCES author(id) ON DELETE CASCADE,
  role TEXT NOT NULL DEFAULT 'editor',
  position INTEGER,
  PRIMARY KEY (book_id, author_id)) WITHOUT ROWID;
CREATE TABLE member (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT,
                     active BOOLEAN NOT NULL DEFAULT 1);
CREATE TABLE loan (
  id INTEGER PRIMARY KEY,
  book_id INTEGER NOT NULL REFERENCES book(id) ON DELETE CASCADE,
  member_id INTEGER REFERENCES member(id) ON DELETE SET NULL,
  loan_period INTEGER NOT NULL DEFAULT 1209600,
  returned INTEGER NOT NULL DEFAULT 0);
CREATE TABLE audit_log (id INTEGER PRIMARY KEY, message TEXT NOT NULL);
-- seed: 2 publishers, 3 authors, 5 books (one with a NULL publisher, two with an
-- audio_length), 4 genres, book_genre links, 3 contributions (one translator with
-- position 1), 3 members, 4 loans (one with a NULL member), 2 audit rows.
```

`example.toml` must parse with **zero diagnostics** here, and must resolve
against `schema.sql` with zero diagnostics (M5's test). It relies on the
editable defaults and states only what differs:

- `version = 1`. Write `[defaults]` with all three modes explicitly
  `editable`, for the reader, with a comment showing the one-line
  read-only alternative.
- `author`:
  - `display_name = "Authors"`, `object_name = "author"`,
    `label = "{last_name}, {first_name}"`,
    `order_by = ["last_name", "first_name"]`, `allow_delete = true`;
  - `first_name` and `last_name`: lengths 1–100;
  - `born`: 1000–2100;
  - `internal_note`: hidden.
- `book`:
  - `object_name = "book"`, `label = "{title}"`, `allow_delete = true`,
    `field_order = ["title", "author_id", "isbn", "format"]`;
  - `title`: max 200;
  - `isbn`: create-only, `pattern = '97[89][0-9]{10}'`,
    `display_name = "ISBN"`, `help`;
  - `author_id` ("Author") and `publisher_id` ("Publisher"): display names
    only;
  - `format`: `allowed` hardcover/paperback/ebook, with labels;
  - `pages`: 1–10000;
  - `price`: 0.0–1000.0;
  - `in_print`: `display_name` only;
  - `audio_length`: `type = "duration"`, `unit = "seconds"`, `min = "1m"`,
    `max = "100h"`, `display_name = "Audiobook length"`;
  - `summary`: multiline, max 4000;
  - `revision`: `mode = "read-only"`.
- `genre`:
  - `object_name = "genre"`;
  - `code`: create-only, `pattern = '[a-z-]+'`, max 20;
  - the default label is `{name}`, and delete is not allowed (default).
- `publisher`: `display_name = "Publishers"`, `object_name = "publisher"`,
  `mode = "pickable"` (a lookup table: not listed, picked from, linked to,
  opened read-only).
- `member`:
  - `object_name = "member"`, `allow_delete = true`;
  - `name`: min 1;
  - `email`: `pattern = '[^@\s]+@[^@\s]+'`, max 254.
- `loan`:
  - `object_name = "loan"`, `allow_delete = true`;
  - `book_id` ("Book") and `member_id` ("Member"): display names;
  - `loan_period`: `type = "duration"`, `unit = "seconds"`, `min = "1d"`,
    `max = "90d"`, `display_name = "Loan period"`;
  - `returned`: `type = "bool"`.
- `audit_log`: hidden.
- `[many_to_many.book_genre]`: `headings = { book = "Genres", genre =
  "Books" }`. Detected anyway; the entry only names the headings.
- `[many_to_many.contribution]`: `headings = { book = "Contributors", author
  = "Contributions" }`. Declared because it has extra columns.
  - `[tables.contribution.fields.role]`: `allowed`
    editor/translator/illustrator, with labels.
  - `[tables.contribution.fields.position]`: 1–99.

Comment the file so it teaches the format.

`starter.toml` is the commented starter file that developer mode's **Edit
config** writes when no config exists (a later milestone embeds it with
`include_str!`). It has `version = 1` and nothing else active: `[defaults]`,
a table, fields, durations, `pickable` and a many-to-many entry are all shown
**commented out**, each with a one-line explanation, and a header comment
naming the file's location, saying the app reloads it automatically, and
pointing to `docs/dbmanager-config.md`. It must parse with **zero
diagnostics** to exactly `Config::default()`'s behaviour (built-in editable
defaults, no entries), so creating it changes nothing until the user edits
it.

## Files

Create:

- `crates/fm-dbmanager/src/config/{mod.rs, model.rs, raw.rs (or similar),
  validate.rs, tests.rs}`;
- `docs/dbmanager-config.md`;
- `examples/dbmanager/schema.sql`;
- `examples/dbmanager/example.toml`;
- `examples/dbmanager/starter.toml`.

Edit:

- `crates/fm-dbmanager/Cargo.toml` (`toml`, `regex`, `ts-rs`);
- `src/lib.rs`;
- `Cargo.lock`;
- `CLAUDE.md` (layout: `src/config/`, `examples/dbmanager/`,
  `docs/dbmanager-config.md`).

Do not touch the shell, the frontend, other crates, or `docs/plans/`.

## Acceptance criteria

- `parse(include_str!("../../../../examples/dbmanager/example.toml"))`
  (adjust the relative path) yields zero diagnostics and the expected
  entries.
- `examples/dbmanager/schema.sql` runs on an empty database in a temp dir
  with `foreign_keys = ON` and inserts its seed rows.
- `parse("")` gives `Defaults` with all three modes `Editable`, no entries,
  and a single warning (missing `version`).
- Every rule under "Checked here" / "Checked from the config alone" has a
  test. Each produces an error at the right 1-based line and column, with the
  entry `Rejected`, and `hidden` set only when the entry's own valid `mode`
  was `hidden`.
- A syntax error, and `version = 2`, each give `Config::fail_closed()`
  (all defaults read-only) plus one error, and `fail_closed` holds a short
  reason. Every other outcome (empty text, invalid `[defaults]`, rejected
  entries) has `fail_closed: None`.
- `object_name` parses on a table; an empty one is an error.
- An invalid `[defaults]` gives an error and read-only defaults, and the
  other valid entries are kept.
- `allow_create`, `allow_delete` and `mode` record whether they were set
  explicitly.
- Duration bounds: `"1h30m"`, `"1h 30m"`, `"-2d"`, `"250ms"` and `90` parse.
  `"1m1h"`, `"1h  30m"`, `" 1h"`, `"1.5h"`, `"1H"` and `"h"` are errors.
  `unit` without `type = "duration"` is an error.
- Label templates: `"{a} {b}"` and `"{{literal}} {a}"` parse. `"{a"`,
  `"a}"` and `"{}"` are errors.
- `allowed` parses in all three forms. Mixed or duplicate values are errors.
- `docs/dbmanager-config.md` contains every section above, the diagnostics
  table, and the example file verbatim. A test asserts that the doc contains
  the example's text, so the two cannot drift.
- fmt, clippy pedantic and `cargo test -p fm-dbmanager` pass.

## Tests proving completion

`src/config/tests.rs`:

- `example_parses_clean`
- `fail_closed_reason_only_when_fail_closed`
- `object_name_key`
- `schema_sql_runs`
- `empty_config_defaults_editable`
- `syntax_error_ignores_config`
- `unsupported_version_ignores_config`
- `missing_version_warns`
- `invalid_defaults_fall_back`
- `explicit_flags_recorded`
- `unknown_key_rejects_entry_with_position`
- `bad_mode_rejects_entry`
- `rejected_hidden_entry_stays_hidden`
- `min_greater_than_max`
- `length_bounds_invalid`
- `duration_bound_grammar`
- `unit_requires_duration_type`
- `pattern_must_compile`
- `allowed_forms`
- `allowed_invalid`
- `label_template_parsing`
- `order_by_forms`
- `m2m_entry_rules`
- `bom_accepted`
- `starter_is_inert` (zero diagnostics, no entries, editable defaults)
- `pickable_mode` (parses on a table; rejected in `[defaults]`)
- `doc_contains_example`

Run `cargo test -p fm-dbmanager`, then the workspace commands in `CLAUDE.md`.

## Out of scope

- Anything that needs the schema: unknown names, kinds, `nullable` vs `NOT
  NULL`, forced read-only, duration precision, many-to-many detection, link
  fields, default labels. That is M5.
- Validating field values: M4.
- ts-rs export and the shell: M4 and M10.

## Risks

- Line and column must count characters the way the editor does. Follow
  `fm-tarpack`'s approach: 1-based, with columns counted in characters.
- After this lands, the doc is the spec's only home. Keep it complete and
  exact, because later milestones point at it.
- Editable defaults make a missing entry permissive. The doc's Defaults
  section must say so plainly, and show the one-line read-only alternative.

## Review round 1

**The first run implemented an outdated draft of this plan.** The approved
scope above is authoritative. Bring the implementation up to it, and apply
the review decisions below, in one run.

### Missing approved scope (add all of it)

- **`TableMode::Pickable`** (serde `"pickable"`):
  - it is valid as a table `mode`;
  - `[defaults] table_mode = "pickable"` is an error that invalidates
    `[defaults]`, so all defaults become read-only;
  - test `pickable_mode`.
- **`examples/dbmanager/starter.toml`:** commented and inert, as specified
  under "Examples", with test `starter_is_inert`.
- **`examples/dbmanager/example.toml`:** `publisher` gets `mode =
  "pickable"`. The doc's copy of the example changes too, so
  `doc_contains_example` keeps passing.
- **`docs/dbmanager-config.md`, "File" section,** as specified above. It
  covers:
  - the fixed location (Windows and other platforms);
  - that the app only reads the file;
  - developer mode's **Edit config** and the starter file;
  - automatic reload, with drafts kept and re-checked;
  - one config for all databases;
  - the "no config in use" notice when the file is missing;
  - the fail-closed cases.
- **The Tables `mode` row** with `pickable`, and **the pickable diagnostics
  rows**, as in the tables above.
- **`CLAUDE.md`:** list `examples/dbmanager/starter.toml` in the layout.

### Review decisions

**R1 — a rejected table keeps its hidden fields** (plan gap).

- `TableEntry::Rejected` gains `fields: Vec<FieldEntry>`, the table's field
  entries as parsed: valid or rejected, each with its own `hidden` flag.
- Test `rejected_table_keeps_field_entries`. In it,
  `[tables.author] mode = "editable"` and `bogus = 1` sit together with
  `[tables.author.fields.internal_note] mode = "hidden"`. Assert:
  - the table is `Rejected`;
  - its `fields` contain `internal_note` with hidden mode;
  - the error is reported at `bogus`.
- The model milestone applies this. A field whose entry asks for `hidden`
  stays hidden, and everything else in the table is read-only.

**R2 — an unknown top-level key narrows the defaults.**

- Any unknown top-level key is an error, as before. It now also narrows all
  three defaults to `read-only`, the same as a wrong-typed `defaults`,
  `tables` or `many_to_many` section. The diagnostics row and the File
  section above already say so.
- Test `unknown_top_level_key_narrows_defaults`. `[table.secrets] mode =
  "hidden"` gives:
  - one error at `table`;
  - `Defaults` all `ReadOnly`;
  - other valid entries kept.

**R3 — `allowed` labels must not be empty.**

- An all-whitespace `label` in an `allowed` inline table is an error that
  rejects the field entry. This matches `display_name`.
- Extend `allowed_invalid` with this case.

**R4 — `CLAUDE.md` alignment.** The `src/config/` layout line is out by two
columns. Align its description column with the neighbouring lines. Only the
whitespace changes.

### Accepted implementer choices (record only; do not change)

- `Option<Located<T>>` for optional settings. It records explicit-vs-inherited
  and the position.
- A wrong-typed top-level section narrows the defaults to read-only.
- A non-integer `version` fails closed.
- NaN, infinite and overflowing numeric bounds are errors.
- Duration `min > max` is checked only when both bounds have the same form.
- A scoped `#[expect(clippy::large_enum_variant, reason = …)]`.
- Cross-key error positions:
  - `min > max` is reported at `min`;
  - `min_length > max_length` at `min_length`;
  - `unit` without a duration type at `unit`.
- The duration grammar lives in `src/config/duration.rs` until the rules
  milestone moves it to `src/rules/duration.rs`.

### Acceptance criteria (review round 1)

- Everything in "Missing approved scope" is present.
- `pickable_mode`, `starter_is_inert`, `doc_contains_example` (with the
  pickable publisher), `rejected_table_keeps_field_entries`,
  `unknown_top_level_key_narrows_defaults` and the extended `allowed_invalid`
  pass, as do all existing tests.
- The doc's File section, Tables `mode` row and diagnostics table match this
  plan.
- `CLAUDE.md` lists `starter.toml`, and the `src/config/` line is aligned.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings` and `cargo test --workspace` pass.
- The diff goes back to the reviewer.

