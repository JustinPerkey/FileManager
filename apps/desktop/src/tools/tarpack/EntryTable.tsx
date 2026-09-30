import { Fragment, memo } from "react";
import { Button } from "../../app/Button";
import type { SessionEntry } from "../../lib/generated/SessionEntry";
import { EntryStatus } from "./EntryStatus";
import { EolMarker } from "./EolMarker";
import { MiddlePath } from "./MiddlePath";

export interface EntryTableProps {
  entries: SessionEntry[];
  failedCount: number;
  entriesWithheld: boolean;
  onBrowse: (id: string) => void;
  onClear: (id: string) => void;
}

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

function Summary({ entries, failedCount }: { entries: SessionEntry[]; failedCount: number }) {
  const total = entries.length;
  const ready = entries.filter((e) => e.status === "ready").length;
  const eol = entries.filter((e) => e.normalizeEol).length;
  const all = ready === total && failedCount === 0;
  return (
    <p className="entry-summary">
      {all ? (
        <>
          All <span className="num">{total}</span> {plural(total, "file", "files")} ready
        </>
      ) : (
        <>
          <span className="num">{ready}</span> of <span className="num">{total}</span>{" "}
          {plural(total, "file", "files")} ready
        </>
      )}
      {failedCount > 0 && (
        <>
          <span aria-hidden="true"> · </span>
          <span className="num">{failedCount}</span> {plural(failedCount, "file", "files")} left out
          (errors)
        </>
      )}
      {eol > 0 && (
        <>
          <span aria-hidden="true"> · </span>
          <span className="num">{eol}</span> {plural(eol, "file converts", "files convert")} line
          endings to LF
        </>
      )}
    </p>
  );
}

function WindowsLocation({ entry }: { entry: SessionEntry }) {
  if (entry.assigned === null) return <span className="entry-table__none">—</span>;
  return (
    <>
      <MiddlePath path={entry.assigned} />
      {entry.status === "missing" && <span className="entry-table__note">(not found)</span>}
    </>
  );
}

/** `textContent` stays exactly `path`; a `<wbr>` after each `/` lets wraps fall at directories. */
function BreakablePath({ path }: { path: string }) {
  const parts = path.split("/");
  return (
    <>
      {parts.map((part, i) => (
        <Fragment key={i}>
          {i < parts.length - 1 ? `${part}/` : part}
          {i < parts.length - 1 && <wbr />}
        </Fragment>
      ))}
    </>
  );
}

const Label = ({ children }: { children: string }) => (
  <span className="entry-table__cell-label" aria-hidden="true">
    {children}
  </span>
);

interface RowProps {
  entry: SessionEntry;
  onBrowse: (id: string) => void;
  onClear: (id: string) => void;
}

function sameEntry(a: SessionEntry, b: SessionEntry) {
  return (
    a === b ||
    (a.id === b.id &&
      a.source === b.source &&
      a.assigned === b.assigned &&
      a.status === b.status &&
      a.targetPath === b.targetPath &&
      a.mode === b.mode &&
      a.modeText === b.modeText &&
      a.owner === b.owner &&
      a.normalizeEol === b.normalizeEol)
  );
}

const EntryRow = memo(
  function EntryRow({ entry, onBrowse, onClear }: RowProps) {
    return (
      <tr>
        <td className="entry-table__status">
          <EntryStatus status={entry.status} />
        </td>
        <td className="entry-table__file">
          {entry.source}
          {entry.normalizeEol && (
            <>
              {" "}
              <EolMarker />
            </>
          )}
        </td>
        <td className="entry-table__windows">
          <Label>Windows location</Label>
          <WindowsLocation entry={entry} />
        </td>
        <td className="entry-table__target">
          <Label>Linux target</Label>
          <span className="mono entry-table__target-path">
            <BreakablePath path={entry.targetPath} />
          </span>
        </td>
        <td className="entry-table__mode">
          <Label>Mode</Label>
          <span className="mono entry-table__nowrap">{entry.modeText}</span>{" "}
          <span className="num entry-table__octal entry-table__nowrap">{entry.mode}</span>
        </td>
        <td className="entry-table__owner">
          <Label>Owner</Label>
          {entry.owner}
        </td>
        <td className="entry-table__actions">
          <Button variant="quiet" onClick={() => onBrowse(entry.id)}>
            Browse…{" "}
            <span className="visually-hidden">for {entry.targetPath}</span>
          </Button>
          <Button
            variant="quiet"
            disabled={entry.status === "unassigned"}
            title="Forget this file (nothing is deleted)"
            onClick={() => onClear(entry.id)}
          >
            Clear{" "}
            <span className="visually-hidden">assigned file for {entry.targetPath}</span>
          </Button>
        </td>
      </tr>
    );
  },
  (a, b) => sameEntry(a.entry, b.entry) && a.onBrowse === b.onBrowse && a.onClear === b.onClear,
);

function Empty({ failedCount, entriesWithheld }: Pick<EntryTableProps, "failedCount" | "entriesWithheld">) {
  if (entriesWithheld || failedCount > 0) {
    return (
      <div className="entry-empty">
        <p>
          {entriesWithheld
            ? "No files are listed. A manifest error hides them until it’s fixed."
            : "No files are listed. Every file in this manifest has errors."}
        </p>
        <p className="entry-empty__next">The errors are listed above. Fix them in your editor, then Reload.</p>
      </div>
    );
  }
  return (
    <div className="entry-empty">
      <p>This manifest lists no files.</p>
      <p className="entry-empty__next">
        Add a <span className="mono">[[file]]</span> entry to the manifest, then Reload.
      </p>
    </div>
  );
}

export function EntryTable({ entries, failedCount, entriesWithheld, onBrowse, onClear }: EntryTableProps) {
  if (entries.length === 0) return <Empty failedCount={failedCount} entriesWithheld={entriesWithheld} />;
  return (
    <div className="entry-list">
      <Summary entries={entries} failedCount={failedCount} />
      <table className="entry-table">
        <caption className="visually-hidden">Files in this package</caption>
        <colgroup>
          {["status", "file", "windows", "target", "mode", "owner", "actions"].map((c) => (
            <col key={c} className={`entry-table__col--${c}`} />
          ))}
        </colgroup>
        <thead>
          <tr>
            <th scope="col">Status</th>
            <th scope="col">File</th>
            <th scope="col">Windows location</th>
            <th scope="col">Linux target</th>
            <th scope="col">Mode</th>
            <th scope="col">Owner</th>
            <th scope="col">Actions</th>
          </tr>
        </thead>
        <tbody>
          {entries.map((e) => (
            <EntryRow key={e.id} entry={e} onBrowse={onBrowse} onClear={onClear} />
          ))}
        </tbody>
      </table>
    </div>
  );
}
