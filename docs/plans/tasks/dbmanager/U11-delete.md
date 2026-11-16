# U11 — Deleting objects, with the impact confirmation

Status: awaiting approval
Project: dbmanager   Depends on: U6 (landed), M10 (landed). Can run in
parallel with U7–U10 once U6 has landed; if it runs after them, keep their
behaviour intact.

## Goal

Let the user delete an object where the config allows it. Deleting always
goes through a confirmation that shows exactly what else the delete affects.
A blocked delete cannot be confirmed, and a changed impact is shown again
before anything happens.

## Context

**The product.** FileManager is a portable Windows desktop app (Tauri v2;
React + TypeScript + Vite in `apps/desktop/src/`). The **Database Manager**
(`dbmanager`) edits a SQLite file as an **object editor**.

Deleting is **destructive**, so it is never one accidental keypress away:

- it is offered only where `TableView.canDelete` is true. It is **off by
  default**, and a table opts in through the config;
- it always shows its impact first:
  - the objects deleted with it through foreign-key cascades;
  - references set to NULL or to their default;
  - many-to-many links removed (and whether their link-field values are
    lost);
  - rows in tables the user cannot see (counts only);
  - whether triggers may do more;
  - whether the database blocks the delete;
- the database is backed up before the session's first write.

Earlier tasks built:

- U2: the store, banners (`notices`), errors (`errorMessage(e, "read" |
  "write")`), and `fileName` (`paths.ts`);
- U3: places, `navigate`, `replacePlace`, the object list (roving focus,
  `h2`), and the sheet header;
- U6: the draft and `dirtyCount()`, `discardDraft()`, sheet banners, and the
  conflict handling patterns.

U1 extended `ConfirmDialog`:

```ts
{ open; title; body /* the aria-describedby <p> */; confirmLabel: string | null;
  cancelLabel?; confirmVariant?: "primary" | "danger"; confirmIcon?; confirmBusy?;
  size?: "default" | "wide"; children? /* scrolling content, not in the description */;
  onConfirm; onCancel }
```

Focus starts on Cancel, Escape is Cancel, and focus returns to the opener.

You own `apps/desktop/src/` **except `lib/`**: never edit it. Tools never
import each other.

**Contract rules.**

- The impact is computed by the backend. The UI groups and words it, but
  never recomputes counts.
- `version` and `token` are opaque: pass them back verbatim.
- Hidden tables appear only as counts, with no labels.

### The API used

`lib/dbmanager.ts`:

- `deletePreview(table: string, key: ObjectKey): Promise<DeleteImpact>`;
- `deleteObject(table: string, key: ObjectKey, version: string, impactToken:
  string): Promise<DeleteResult>`. **Always call `deletePreview` first**, and
  pass back its `version` and `token`;
- `isConfirmDeleteAllowed(impact): boolean` (`!impact.blocked`).

```ts
type DeleteImpact = { table: string; key: ObjectKey; label: string; version: string;
  groups: ImpactGroup[];          // breadth-first: depth, then table, then via
  blocked: boolean; truncated: boolean; hasTriggers: boolean; token: string };
type ImpactGroup = { table: string; tableDisplayName: string; hidden: boolean;
  via: string;                    // FK field display name(s); SQL names for hidden tables
  action: "cascade" | "setNull" | "setDefault" | "restrict" | "noAction";
  count: number; depth: number;   // 1 = references the object directly
  truncated: boolean;             // count is a lower bound
  sample: string[];               // up to 10 labels; empty when hidden
  links: boolean;                 // junction rows (many-to-many links); tableDisplayName is then the relation heading
  linkData: boolean;              // those links have link-field values, which are lost
  blocks: boolean };
type DeleteResult = { table: string; label: string; impact: DeleteImpact; backupCreated: string | null };
```

Errors from `deleteObject`:

- `ImpactChanged` (with `impact`, the fresh impact; nothing deleted);
- `DeleteBlocked` (with `impact`);
- `Conflict` (with `current`: the object changed since the preview);
- `NotEditable`, `DatabaseReadOnly`, `ConfigChangedOnDisk`, `SchemaChanged`,
  `Busy`, `BackupFailed`, `ConstraintFailed`, `UnknownObject`, `Io`.

`deletePreview` can fail with the read errors.

### Grouping (`impact.ts`, pure, unit-tested)

`describeImpact(impact): { sections: Section[]; otherRows: number;
otherRowsAtLeast: boolean }`. `otherRows` is the sum of every group's
`count`; it is "at least" when any group or the impact is truncated. The
sections, in this order, contain only the groups that apply:

| Section heading | Groups | Line |
| --- | --- | --- |
| **Blocking the delete** | `blocks` | visible: "{count} objects in {tableDisplayName} (via {via}) still reference it: {samples}"; hidden: "{count} rows in {tableDisplayName} (not shown in this tool) still reference it" |
| **Also deleted** | `cascade`, `!links`, `!hidden` | "{count} objects in {tableDisplayName} (via {via}): {samples}" |
| **Links removed** | `cascade`, `links`, `!hidden` | "{tableDisplayName}: {count} links ({samples})." + when `linkData`: " The values stored on these links are lost." |
| **Reference cleared** | `setNull`, `!blocks`, `!hidden` | "{count} objects in {tableDisplayName} will have {via} cleared: {samples}" |
| **Reference reset to its default** | `setDefault`, `!hidden` | "{count} objects in {tableDisplayName} will have {via} reset to its default: {samples}" |
| **Not shown in this tool** | `hidden`, `!blocks` | "{count} rows in {tableDisplayName} (via {via}) are deleted / cleared / reset" (by action) |

**Wording rules:**

- `{count}` is "at least {count}" when the group is truncated. "objects",
  "rows" and "links" become singular for a count of 1.
- `{samples}` is the samples joined by ", ", plus " and {count −
  sample.length} more" when the count is larger. A line with no samples ends
  at the colon-less form.
- Groups with `depth ≥ 2` end " (through the objects above)".
- Numbers use `.num`.

### The design (brief excerpt)

**Visual system** (`DESIGN.md`; no new tokens):

- U1's **danger** button variant (`--danger` text and border on
  `--surface`) is used only for **Delete…** and the dialog's **Delete**.
  It is not primary;
- the wide `ConfirmDialog` on `--surface`, with `--shadow-overlay` and
  `--scrim`;
- warn notices: the `alert-triangle` icon in `--warn` plus words;
- tokens: `--surface`, `--surface-sunken`, `--border`, `--text`,
  `--text-muted`, `--danger`, `--warn`, the spacing and type scales.

**Starting a delete.**

- **Delete…** sits in the sheet header, at the right of the title, away from
  Save. It is a danger `Button` with the `trash` icon.
- It is shown only when `table.canDelete`, the session is writable (no
  `readOnlyReason`, no `configFailClosed`), and the place is an existing
  object (never on the create form).
- **The Delete key** on a focused object-list item (U3's roving list) starts
  the same flow for that item, under the same conditions. It never deletes
  directly.
- **While the preview loads,** the button is busy with "Checking…" (for the
  key path, announce "Checking what this delete affects…"). A preview error
  shows a sheet banner with `errorMessage(e, "read")`, and nothing opens.

**The dialog** (`DeleteDialog`, wrapping `ConfirmDialog` with `size="wide"`):

- **Confirmable** (`!impact.blocked`):
  - title: "Delete “{impact.label}”?";
  - `body` (the description): "This deletes it from {table display
    name}{otherRows > 0 ? " and changes {at least }{otherRows} other rows" :
    ""}.", then:
    - with no `session.backupPath`: "A backup of the database is saved
      first.";
    - with one: "This session's backup is {fileName(backupPath)}.";
  - `children`:
    - the sections, each an `h3` (`--font-size-md`, weight 600) with a `ul`
      of lines; or "Nothing else references it." when there are none;
    - when `hasTriggers`: a warn line, "This database has triggers. They may
      change or delete more than is listed here.";
    - when the target is the open object with a dirty draft: "Your unsaved
      changes to it are discarded.";
  - buttons: **Cancel** (focused) and **Delete** (`confirmVariant="danger"`,
    `confirmIcon="trash"`).
- **Blocked** (`impact.blocked`):
  - title: "“{impact.label}” can't be deleted";
  - body: "Other objects still reference it, and the database does not allow
    deleting it while they do. Change or delete those first.";
  - `children`: the **Blocking the delete** section first, then the `h3` "If
    it could be deleted, this would also happen:" and the remaining
    sections;
  - `confirmLabel={null}` and `cancelLabel="Close"`: one button, focused.
- **Deleting:** `confirmBusy`. The button reads "Deleting…", Cancel is
  disabled, and Escape is ignored (U1's dialog behaviour).

**Outcomes.**

| Result | What happens |
| --- | --- |
| success | close the dialog. If the place is the deleted object, `replacePlace({ kind: "table", table })`, discard its draft, and move focus to the object list's `h2`. Push the notice "Deleted “{label}”{ and {at least }{otherRows} related rows}." and, when `backupCreated`, the notice "A backup of the database was saved before this first change: {backupCreated}." plus a session re-read. Re-read the object list. Announce the deleted sentence. |
| `ImpactChanged` | keep the dialog open with `error.impact` (new token and version). Show a warn notice at the top of `children`: "What this delete affects changed. Review it again." Move focus to that notice (`tabIndex={-1}`). |
| `DeleteBlocked` | keep the dialog open, switched to the blocked state with `error.impact` |
| `Conflict` | close the dialog. Show `error.current` in the sheet (as the base; rebase a draft through U6's `rebaseDraft`), and a sheet banner: "“{label}” was changed by another program. Nothing was deleted. Review it and try again." |
| `ConfigChangedOnDisk`, `SchemaChanged` | close the dialog, then U6's handling (session re-read / schema flag), with `errorMessage(e, "write")` in a sheet banner |
| `Busy` | close the dialog. A sheet banner with `errorMessage(e, "write")` and **Try again**, which restarts from the preview. |
| any other error | close the dialog. A sheet banner with `errorMessage(e, "write")` and Details. |

A history entry for the deleted object is kept. Opening it later shows U3's
"This object no longer exists" state.

**Shortcut row added** to `shortcuts.ts`:

| Keys | Action | Active when |
| --- | --- | --- |
| Delete | Delete the focused object… (asks first) | Focus is on an object in the list, and the table allows deleting |

## Files

Create in `apps/desktop/src/tools/dbmanager/`:

- `DeleteDialog.tsx`, `impact.ts`;
- tests: `impact.test.ts`, `DeleteDialog.test.tsx`, `delete.flow.test.tsx`.

Edit `ObjectSheet.tsx` (Delete…), `ObjectList.tsx` (the Delete key),
`store.ts` (the delete flow), `shortcuts.ts`, `fixtures.ts` and
`styles/dbmanager.css`.

**Fixtures** (from the example data).

The author "Lovelace, Ada" has this impact:

| Group | Action | Depth | Count / samples | Flags |
| --- | --- | --- | --- | --- |
| `book` | cascade | 1 | 2: "Dune", "Emma" | |
| `contribution` ("Contributions") | cascade | 1 | 1: "Persuasion" | `links`, `linkData` |
| `book_genre` ("Genres") | cascade | 2 | | `links`, no `linkData` |
| `loan` | cascade | 2 | 3 | |
| `contribution` of the deleted books | cascade | 2 | | `links`, `linkData` |
| hidden `audit_log` | cascade | 1 | 2 | `hidden` |

The other fixtures:

- the publisher "Acme Press": `book` restrict, count 3, `blocks`,
  `blocked: true`;
- a member: `loan` setNull, count 2;
- a truncated cascade: count 10000, `truncated`;
- `hasTriggers: true`;
- an object with no impact.

## Skill

- Run `/impeccable clarify` on every line of the impact copy and the
  outcomes (keep the meaning; report any proposed change), then
  `/impeccable harden` on the flow: slow previews, impact changes, conflicts
  and busy. Run both from `apps/desktop` with `--target
  src/tools/dbmanager/DeleteDialog.tsx`.
- Read `reference/craft-floor.md` before the first UI edit.
- Extend the established world. The Schedule view's confirm dialog is the
  incumbent pattern.
- Run `/impeccable audit` and the detector before done.
- If the skill is missing, install it (`npx impeccable install`) or follow
  its references by hand, and say so.

## Acceptance criteria

**Where Delete is offered:**

- Delete… appears on author, book, member and loan objects (`canDelete`).
- It does not appear on genre objects, on any create form, or anywhere when
  the session is read-only.

**Starting:**

- Delete… shows "Checking…", calls `deletePreview(table, key)`, then opens
  the dialog with focus on Cancel.

**The Lovelace impact** renders:

- the description "This deletes it from Authors and changes 9 other rows. A
  backup of the database is saved first." (the counts follow the fixture);
- the sections **Also deleted** (books, with samples), **Links removed**
  (Contributions with the lost-values sentence; Genres without it; the
  depth-2 groups end "(through the objects above)"), and **Not shown in this
  tool** (audit_log: a count, no samples);
- the triggers line when `hasTriggers`.

**Confirming:**

- Delete calls `deleteObject(table, key, impact.version, impact.token)`.
- While pending: Deleting…, with Cancel and Escape disabled.
- On success: the place becomes the table, focus is on the list `h2`, the
  deleted notice and the backup notice show, and the list re-reads.

**Blocked:**

- "Acme Press" opens the blocked dialog: the title "“Acme Press” can't be
  deleted", the Blocking section first, and only Close (focused).

**Errors:**

- `ImpactChanged` replaces the content, focuses the notice, and the next
  Delete uses the new token.
- `DeleteBlocked` switches the dialog to blocked.
- `Conflict` closes the dialog and shows the sheet banner with `current`.
- `Busy` shows Try again, which restarts from the preview.

**Copy:**

- A truncated count reads "at least 10000".
- Singular counts are grammatical.
- "and N more" appears when the count exceeds the samples.
- With a dirty draft on the target, the discard sentence appears.

**Keyboard:**

- The Delete key on a focused list item opens the same dialog. Closing it
  returns focus to that item.
- The Delete key in a text input never starts a delete.

**Accessibility:**

- The dialog is labelled by its title and described by the short `body` only.
- The impact list is in a scrollable region reachable by keyboard.
- Nothing is conveyed by colour alone.
- jsx-a11y lint passes.
- At 800×560 and 200% text, the dialog fits the window and its content
  scrolls inside it.

## Tests proving completion

- `impact.test.ts`:
  - `sections and order`;
  - `link data sentence`;
  - `hidden counts without samples`;
  - `depth suffix`;
  - `truncation and at least`;
  - `singular and more`;
  - `otherRows sum`.
- `DeleteDialog.test.tsx`:
  - `confirmable content and focus`;
  - `backup sentence by session`;
  - `blocked has only close`;
  - `triggers and dirty lines`;
  - `no impact`;
  - `busy state`.
- `delete.flow.test.tsx`:
  - `offered only when allowed`;
  - `preview then confirm calls deleteObject with version and token`;
  - `success navigates, notices, focus`;
  - `impact changed`;
  - `delete blocked`;
  - `conflict`;
  - `busy retry`;
  - `delete key from the list`;
  - `delete key ignored in inputs`.

Run: `npm run typecheck && npm run lint && npm run test`.

## States covered

- **Loading:** checking (preview), deleting.
- **Empty:** no impact.
- **Error:** preview error, blocked, impact changed, conflict, busy, any
  other write error.
- **Success:** deleted, with and without a first backup.

## Out of scope

- Batch delete (not in v1).
- Predicting trigger effects (flagged only).
- Undo after delete: recovery is the backup, which the notice names.
