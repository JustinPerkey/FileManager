# Database Manager config reference

This is the reference for the Database Manager's config file, for someone who
edits it by hand. It is the format's only specification.

## Purpose

The Database Manager opens a SQLite file. Table structure, column types and
foreign keys come from the database itself. The config is an **overlay** on
that structure. It says which tables, fields and many-to-many relations are
editable, read-only, hidden or create-only, sets field bounds, and declares
labels, durations and many-to-many relations.

**Anything the config does not mention is editable**, except the columns that
can never be edited (see "Forced read-only"). A missing entry is therefore
permissive. To make a database read-only unless stated otherwise, write
`table_mode = "read-only"` (see "Defaults").

Some rules need the database (names that exist, kinds, `nullable` against
`NOT NULL`, and so on). They are checked when the config is applied to a
database. Every rule is documented here, whichever step checks it.

## Complete example

This is `examples/dbmanager/example.toml`, which goes with
`examples/dbmanager/schema.sql`.

```toml
# Database Manager config for the library database in schema.sql.
#
# The database supplies table structure, column types and foreign keys. This
# file is an overlay: it says what is editable, read-only or hidden, and adds
# labels, bounds and durations. Anything it does not mention is editable
# (except columns that can never be edited, such as a rowid).

version = 1

# What applies to every table, field and many-to-many relation not mentioned
# below. These are the built-in values, written out for the reader. To make the
# whole database read-only unless stated otherwise, use one line instead:
#   table_mode = "read-only"
[defaults]
table_mode = "editable"
field_mode = "editable"
relation_mode = "editable"

# --- author ---------------------------------------------------------------
[tables.author]
display_name = "Authors"
object_name = "author"                     # "New author"
label = "{last_name}, {first_name}"        # an author's title in lists and links
order_by = ["last_name", "first_name"]
allow_delete = true                        # off unless asked for

[tables.author.fields.first_name]
min_length = 1
max_length = 100

[tables.author.fields.last_name]
min_length = 1
max_length = 100

[tables.author.fields.born]
min = 1000
max = 2100

[tables.author.fields.internal_note]
mode = "hidden"                            # never shown

# --- book -----------------------------------------------------------------
[tables.book]
object_name = "book"
label = "{title}"
allow_delete = true
field_order = ["title", "author_id", "isbn", "format"]   # these first

[tables.book.fields.title]
max_length = 200

[tables.book.fields.isbn]
mode = "create-only"                       # set when creating, read-only after
display_name = "ISBN"
help = "13 digits, starting 978 or 979."
pattern = '97[89][0-9]{10}'                # the whole value must match

[tables.book.fields.author_id]
display_name = "Author"

[tables.book.fields.publisher_id]
display_name = "Publisher"

[tables.book.fields.format]
allowed = [
  { value = "hardcover", label = "Hardcover" },
  { value = "paperback", label = "Paperback" },
  { value = "ebook", label = "E-book" },
]

[tables.book.fields.pages]
min = 1
max = 10000

[tables.book.fields.price]
min = 0.0
max = 1000.0

[tables.book.fields.in_print]
display_name = "In print"

# A duration is stored as an integer count of its unit (here seconds) and
# shown as, for example, 7h30m.
[tables.book.fields.audio_length]
type = "duration"
unit = "seconds"
min = "1m"
max = "100h"
display_name = "Audiobook length"

[tables.book.fields.summary]
multiline = true
max_length = 4000

[tables.book.fields.revision]
mode = "read-only"

# --- genre ----------------------------------------------------------------
# No label: the default is the `name` column. Delete is off by default.
[tables.genre]
object_name = "genre"

[tables.genre.fields.code]
mode = "create-only"
pattern = '[a-z-]+'
max_length = 20

# --- publisher ------------------------------------------------------------
# A lookup table: books can pick a publisher, but nothing here changes. It is
# not listed on its own.
[tables.publisher]
display_name = "Publishers"
object_name = "publisher"
mode = "pickable"                          # not listed; picked from, linked to, opened read-only

# --- member ---------------------------------------------------------------
[tables.member]
object_name = "member"
allow_delete = true

[tables.member.fields.name]
min_length = 1

[tables.member.fields.email]
pattern = '[^@ \t\r\n]+@[^@ \t\r\n]+'
max_length = 254

# --- loan -----------------------------------------------------------------
[tables.loan]
object_name = "loan"
allow_delete = true

[tables.loan.fields.book_id]
display_name = "Book"

[tables.loan.fields.member_id]
display_name = "Member"

[tables.loan.fields.loan_period]
type = "duration"
unit = "seconds"
min = "1d"
max = "90d"
display_name = "Loan period"

[tables.loan.fields.returned]
type = "bool"                              # an INTEGER column holding 0 or 1

# --- audit_log ------------------------------------------------------------
[tables.audit_log]
mode = "hidden"

# --- many-to-many ---------------------------------------------------------
# book_genre is detected as many-to-many without any entry; this one only names
# the checklist headings (keyed by the table each checklist appears on).
[many_to_many.book_genre]
headings = { book = "Genres", genre = "Books" }

# contribution has extra columns, so it must be declared. Its other columns
# (role, position) are the link fields, configured like any field.
[many_to_many.contribution]
headings = { book = "Contributors", author = "Contributions" }

[tables.contribution.fields.role]
allowed = [
  { value = "editor", label = "Editor" },
  { value = "translator", label = "Translator" },
  { value = "illustrator", label = "Illustrator" },
]

[tables.contribution.fields.position]
min = 1
max = 99
```

## File

There is **one config for the whole app**, at a fixed location the user does
not choose:

- Windows: `%ProgramData%\FileManager\dbmanager.toml` (normally
  `C:\ProgramData\FileManager\dbmanager.toml`);
- other platforms (development builds only): `/etc/filemanager/dbmanager.toml`.

The app only reads it; it never writes it, except that developer mode
(Ctrl+Alt+Shift+D) offers **Edit config**, which opens the file in the system
editor and, if the file does not exist yet, first creates a commented starter
file there (never replacing an existing file). The starter file is
`examples/dbmanager/starter.toml`; it changes nothing until edited. Saved edits
are picked up **automatically**: the app watches the file and reloads it,
keeping unsaved edits in the form and re-checking them against the new rules. A
save that was prepared against the old config is refused once and can be
retried.

The same config applies to every database the app opens. Table and field names
that a database does not have are warnings, not errors, so one file can
describe several databases.

UTF-8 TOML, optional BOM. Top-level keys: `version`, `defaults`, `tables`,
`many_to_many`.

Any other top-level key (for example the typo `[table.secrets]`) is an error,
is ignored, and **narrows all three defaults to `read-only`**, as a wrong-typed
`defaults`, `tables` or `many_to_many` does. A misspelt section could otherwise
leave the tables it meant to restrict editable. Entries that parsed correctly
still apply.

- `version = 1`. If it is missing: a warning, read as 1. Any other value: an
  error. The whole config is ignored and **everything is read-only** until it
  is fixed.
- A TOML syntax error is an error at its position. The whole config is
  ignored and **everything is read-only** until it is fixed.
- A config file that cannot be read (I/O error, not UTF-8): everything is
  read-only.
- No config file at all: the built-in defaults apply, so everything is
  editable except forced read-only columns, and the app says that no config is
  in use.

Positions in diagnostics are 1-based lines and columns, with columns counted in
characters.

## Defaults

```toml
[defaults]
table_mode = "editable"      # editable | read-only | hidden
field_mode = "editable"      # editable | read-only | hidden
relation_mode = "editable"   # editable | read-only | hidden
```

All three default to `editable`. They apply to every table, field and
many-to-many relation the config does not mention. **A missing entry is
therefore editable.** To make a database read-only unless stated otherwise,
write one line:

```toml
[defaults]
table_mode = "read-only"
```

`create-only` and `pickable` are not valid defaults. An invalid `[defaults]` (unknown key or
bad value) is an error, and all three defaults become `read-only` (fail
closed). The entries that are valid still apply.

Whatever the defaults say, some columns are never editable (see "Forced
read-only"), and an entry with an error makes its subject read-only.

## Tables: `[tables.<name>]`

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

Errors in a table's own keys reject the table entry (see "Diagnostics"); its
fields are then read-only, except that a field whose entry validly says
`mode = "hidden"` stays hidden, so a typo never reveals a hidden column. An error in one field entry rejects
only that field.

## Fields: `[tables.<table>.fields.<column>]`

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
- a bound that is not finite (`inf`, `nan`) or an integer outside the 64-bit
  range;
- a duration bound string that does not match the duration grammar;
- a `pattern` that Rust's `regex` crate cannot compile when anchored;
- `allowed` that is empty, mixes strings and integers, contains a duplicate
  value, has an inline table with keys other than `value`/`label`, is
  missing a `value`, or has an empty (all-whitespace) `label`;
- an empty (all-whitespace) `display_name`, `help`, `description` or
  `object_name`.

Pattern dialect: Rust's `regex` is authoritative on save, while the UI checks
the same pattern with JavaScript `RegExp` (`u` flag) as the user types. The two
read some constructs differently and accept different syntax, so a pattern
must stay in their common subset. Anything outside it is an **error** (the
pattern is dropped and the field is read-only). The allowed subset is exactly:

- literal characters, groups `( )` and non-capturing groups `(?: )`,
  alternation `|`, and the quantifiers `* + ? {n} {n,} {n,m}`;
- character classes `[...]` and `[^...]`, with ranges;
- escapes: `\\ \^ \$ \. \* \+ \? \( \) \[ \] \{ \} \| \/`, `\t \n \r \f \v`
  and `\xHH` (two hex digits). `\-` is allowed only inside a class.

Rejected, and why:

- The shorthand classes `\d \D \w \W \s \S \b \B`: the engines disagree on
  non-ASCII input. Rust's are Unicode-aware (`\d` matches Arabic-Indic digits),
  JavaScript's `\d \w \b` are ASCII-only, and the two `\s` sets differ. Write
  explicit classes instead: `[0-9]`, `[A-Za-z0-9_]`, `[ \t\r\n]`.
- Silent meaning differences: an unescaped `[` inside a class (a nested
  class), POSIX classes (`[[:alpha:]]`), and the set operators `&&`, `--` and
  `~~` inside a class.
- Syntax one engine refuses: inline flags and look-around (`(?` followed by
  anything other than `:`, so `(?i)`, `(?=`, `(?!`, `(?<`), `\A \z \Z`, `\p` and
  `\P` in any form, back-references `\1` to `\9`, and any other escape (`\@` is
  a SyntaxError in JavaScript's `u` mode).
- Punctuation that Rust takes literally and JavaScript's `u` mode rejects: a
  `]` straight after `[` or `[^` (write `\]`), an unescaped `]` or `}` outside
  a class (write `\]`, `\}`), a `{` that does not start a valid `{n}`,
  `{n,}` or `{n,m}` quantifier with ASCII digits and `n <= m` (write `\{`), and
  `\-` outside a class (write `-`).

An unescaped `.` outside a class is a **warning**: JavaScript's `.` also
excludes `\r`, U+2028 and U+2029, Rust's does not. Write `[^\n]` for "any
character". The pattern still applies.

## Kinds

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

## Durations

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

A bound string is checked against the grammar when the config is read. Whether
it is a whole number of the field's unit is checked once the field's unit is
known.

## Forced read-only

These are always read-only, whatever the config says:

- a rowid alias (`INTEGER PRIMARY KEY`, assigned by SQLite);
- generated columns;
- `blob` columns, and untyped/`any` columns without `type`;
- foreign keys of more than one column;
- foreign keys into a hidden table, or into a table that is not listed (a
  junction used as many-to-many): there is no link, so the column is a plain
  read-only field;
- foreign keys whose target is not the parent's primary key or a unique key
  (SQLite refuses changes that touch them).

The primary-key columns of an existing object are never editable either. An
inherited `editable` on one becomes `create-only`, so it can be set when
creating.

Asking explicitly for `editable` (or `create-only`, except on a primary-key
column) on one of these is an error. Inheriting it from `[defaults]` silently
narrows it.

## Labels

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

**Hidden identity.** If any identity column of the table is hidden (a hidden
`INTEGER PRIMARY KEY` alias, or a hidden primary-key column of a `WITHOUT
ROWID` table), the fallback is the display name alone, so the hidden value
never shows. A rowid table without an alias column always shows its rowid,
which cannot be hidden. Give any table whose id is hidden a `label`, or its
objects all look alike in lists. Hiding a key column does not hide copies of
its value in other tables: a foreign key column that references it shows the
value unless that column is hidden too.

Search in a table's object list matches the label's non-duration columns
(substring, case-insensitive for ASCII). A table whose label has no such
columns is not searchable.

Checked from the config alone: an unbalanced `{` or `}`, or an empty `{}`, is
an error, and the table entry is rejected.

## Foreign keys and reverse relations

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

## Many-to-many: `[many_to_many.<junction table>]`

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

A junction configured more than once (names match without case): each entry
after the first is a warning. If any entry says `hidden`, the relation is
hidden (and `enabled = false` is ignored). Otherwise the relation is kept,
read-only, from the first valid entry that declares `columns`, or else the
first valid entry, so a declared junction is not turned into an editable
table.

Checked from the config alone: an error rejects the entry, and its relation
is read-only. If the table is detected without the entry (no extra columns),
it stays many-to-many, read-only. If the entry was needed to declare it, the
relation cannot be used safely (its `columns` may be the broken key), so the
table is an ordinary **listed table forced read-only**: all its fields are
read-only, with no create or delete. It is not hidden, because the entry did
not validly ask for that. The errors are:

- `columns` that is not exactly two distinct strings;
- a `headings` value that is not a non-empty string;
- unknown keys.

## Diagnostics

Errors never make anything more editable than its entry would have been. A
rejected entry leaves its subject read-only (or hidden, if it validly asked
for hidden). The database always opens and can be browsed.

| Situation | Severity | Effect |
| --- | --- | --- |
| TOML syntax error, `version` ≠ 1 | error | whole config ignored; everything read-only (fail closed) |
| invalid `[defaults]` | error | all defaults `read-only` |
| missing `version` | warning | read as 1 |
| unknown top-level key, or a wrong-typed `defaults`/`tables`/`many_to_many` | error | ignored; all defaults narrowed to `read-only`; valid entries still apply |
| unknown key, wrong type, invalid value (any rule above) | error | that table/field/relation entry rejected: its subject is read-only (hidden if it validly asked for hidden); a rejected table keeps its fields' `hidden` |
| unknown table, field, or junction | warning | entry ignored |
| names a view, virtual, shadow or internal table | warning | entry ignored |
| bound, `type` or `unit` not valid for the field's kind or affinity, or set on a foreign-key field | error | field read-only |
| duration bound not a whole number of the unit | error | field read-only |
| `pattern` has a `]` straight after `[`/`[^`, an unescaped `]` or `}` outside a class, a `{` that is not a valid quantifier, or `\-` outside a class | error | pattern dropped; field read-only |
| `pattern` uses a shorthand class (`\d \D \w \W \s \S \b \B`), a construct outside the common subset (nested class, POSIX class, `&&`/`--`/`~~` in a class, inline flag or look-around, `\A \z \Z`, `\p \P`, back-reference, other escape), or does not compile | error | pattern dropped; field read-only |
| `pattern` has an unescaped `.` outside a class | warning | pattern still applies |
| `nullable = true` on a `NOT NULL` column | error | field read-only |
| `editable` on a foreign key whose target is not a primary or unique key | error | field read-only |
| `multiline` on a non-text field | warning | ignored |
| `empty_is_null` on a non-nullable or non-text field | error | field read-only |
| the same table, field or junction configured more than once (names match without case) | warning on each entry after the first | hidden if any entry hides it; otherwise treated as a rejected entry (read-only table, field or relation) |
| explicit `editable`/`create-only` on a forced-read-only column | error | field read-only |
| field explicitly `editable`/`create-only` in a read-only or pickable table | warning | stays read-only |
| `allow_create`/`allow_delete` set explicitly on a non-editable (read-only, pickable, hidden) table | warning | ignored |
| `table_mode = "pickable"` in `[defaults]` | error | `[defaults]` invalid (all defaults read-only) |
| `mode = "pickable"` on a junction used as many-to-many | warning | treated as not listed |
| a `NOT NULL` column that is neither `editable` nor `create-only`, has no default and is not a rowid alias, while create is on | error if `allow_create` was explicit, warning if inherited | create disabled |
| the primary key a create must supply (every column of a `WITHOUT ROWID` table's key, even with a default; a rowid table's declared non-alias key column without a default) has a column that is hidden, read-only or generated, while create is on | warning, at the column's entry, else the table's | create disabled |
| a junction `NOT NULL` column without default that no editable/create-only link field supplies, while the relation is editable | error if the relation `mode` was explicit, warning if inherited | relation read-only |
| label/order_by/field_order names an unknown or hidden column | error | that key ignored, default used |
| FK to a missing table/column | warning | plain field |
| FK to a column that is not a primary or unique key | warning | plain read-only field |
| self-referential junction declared | warning | not many-to-many |
| table with no usable identity | warning | table skipped |

Config diagnostics carry line and column. Database diagnostics carry the
table and column.

Diagnostics may name any table or column, hidden ones included. Views and
values never do. Diagnostics describe the config and the schema to whoever
maintains them, and the config file already names those columns; `hidden`
controls what object data the user sees, not whether the schema is secret.
(Warnings about foreign keys are only given for visible tables and columns.)
