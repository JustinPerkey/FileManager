import { Button } from "../../app/Button";
import { Icon, type IconName } from "../../app/icons";
import type { Ambiguity } from "../../lib/generated/Ambiguity";
import type { DropOutcome } from "../../lib/generated/DropOutcome";
import type { SessionEntry } from "../../lib/generated/SessionEntry";
import type { UnmatchedReason } from "../../lib/generated/UnmatchedReason";

type Tone = "ok" | "warn" | "muted";

interface ReasonCopy {
  phrase: (n: number, hasFailedEntries: boolean) => string;
  icon: IconName;
  tone: Tone;
}

const one = (n: number, singular: string, many: string) => (n === 1 ? singular : many);

/** Copy per reason, in display order. A `Record`: a reason added in Rust fails the typecheck until it has copy. */
const reasons: Record<UnmatchedReason, ReasonCopy> = {
  noEntry: {
    phrase: (_, failed) => (failed ? "not matched" : "not in the manifest"),
    icon: "info",
    tone: "muted",
  },
  alreadyAssigned: {
    phrase: () => "already assigned, left unchanged",
    icon: "info",
    tone: "muted",
  },
  folderNoMatch: {
    phrase: (n) => one(n, "folder with nothing to match", "folders with nothing to match"),
    icon: "info",
    tone: "muted",
  },
  linkNotFollowed: {
    phrase: (n) => one(n, "link not followed", "links not followed"),
    icon: "info",
    tone: "muted",
  },
  notFound: {
    phrase: () => "no longer found",
    icon: "alert-triangle",
    tone: "warn",
  },
  unreadable: {
    phrase: () => "could not be read",
    icon: "alert-triangle",
    tone: "warn",
  },
  notUnicode: {
    phrase: (n) =>
      n === 1
        ? "not assigned, unsupported characters in its path \u2014 rename it or its folder"
        : "not assigned, unsupported characters in their paths \u2014 rename them or their folders",
    icon: "alert-triangle",
    tone: "warn",
  },
};
const ORDER = Object.keys(reasons) as UnmatchedReason[];

const HINT = "Files for entries with errors can't be matched until those errors are fixed.";
const EMPTY = "Nothing was matched";
const MAX_NAMES = 8;

interface Item {
  label: string;
  title: string;
  mono?: boolean;
  /** Parenthesised note after the label: `{ count }` renders as "({count} files)". */
  note?: { count: number } | { text: string };
}

interface Line {
  key: string;
  icon: IconName;
  tone: Tone;
  /** Leading count, rendered with `.num`; null for lines without one. */
  count: number | null;
  /** Text after the count, including the colon when items follow. */
  lead: string;
  items: Item[];
  /** Items beyond the cap. */
  more: number;
  /** Every path behind this line, uncapped, for the "Full paths" disclosure. */
  full?:
    | { lead: string; paths: string[] }
    | { lead: string; entries: { label: string; mono: boolean; candidates: string[] }[] };
}

/** The last path segment of a display path. */
function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

function capped(items: Item[]) {
  return { items: items.slice(0, MAX_NAMES), more: Math.max(0, items.length - MAX_NAMES) };
}

function ambiguousItem(a: Ambiguity, byId: Map<string, SessionEntry>): Item {
  const entry = byId.get(a.id);
  const first = a.candidates[0] ?? "";
  const fileName = baseName(first);
  return {
    // An id missing from `entries` should not happen; never show the raw id.
    label: entry ? entry.targetPath : fileName,
    mono: entry !== undefined,
    title: `Matching files:\n${a.candidates.join("\n")}`,
    note:
      a.candidates.length > 1
        ? { count: a.candidates.length }
        : { text: `${fileName} fits more than one entry` },
  };
}

function buildLines(outcome: DropOutcome, entries: SessionEntry[], hasFailedEntries: boolean): Line[] {
  const lines: Line[] = [];
  if (outcome.matched.length > 0) {
    lines.push({
      key: "matched",
      icon: "check-circle",
      tone: "ok",
      count: outcome.matched.length,
      lead: "matched",
      items: [],
      more: 0,
    });
  }
  for (const reason of ORDER) {
    const paths = outcome.unmatched.filter((u) => u.reason === reason).map((u) => u.path);
    if (paths.length === 0) continue;
    const copy = reasons[reason];
    lines.push({
      key: reason,
      icon: copy.icon,
      tone: copy.tone,
      count: paths.length,
      lead: `${copy.phrase(paths.length, hasFailedEntries)}:`,
      full: { lead: `${paths.length} ${copy.phrase(paths.length, hasFailedEntries)}`, paths },
      ...capped(paths.map((p) => ({ label: baseName(p), title: p }))),
    });
  }
  const showHint = outcome.unmatched.some((u) => u.reason === "noEntry" || u.reason === "folderNoMatch");
  if (hasFailedEntries && showHint) {
    lines.push({ key: "hint", icon: "info", tone: "muted", count: null, lead: HINT, items: [], more: 0 });
  }
  if (outcome.ambiguous.length > 0) {
    const n = outcome.ambiguous.length;
    const byId = new Map(entries.map((e) => [e.id, e]));
    lines.push({
      key: "ambiguous",
      icon: "alert-triangle",
      tone: "warn",
      count: n,
      lead: `ambiguous, not assigned \u2014 use Browse\u2026 on ${n === 1 ? "its row" : "their rows"}:`,
      full: {
        lead: `${n} ambiguous, not assigned \u2014 use Browse\u2026 on ${n === 1 ? "its row" : "their rows"}`,
        entries: outcome.ambiguous.map((a) => {
          const item = ambiguousItem(a, byId);
          return { label: item.label, mono: item.mono === true, candidates: a.candidates };
        }),
      },
      ...capped(outcome.ambiguous.map((a) => ambiguousItem(a, byId))),
    });
  }
  if (lines.length === 0) {
    lines.push({ key: "empty", icon: "info", tone: "muted", count: null, lead: EMPTY, items: [], more: 0 });
  }
  return lines;
}

function itemText(i: Item): string {
  if (!i.note) return i.label;
  return `${i.label} (${"count" in i.note ? `${i.note.count} files` : i.note.text})`;
}

function lineText(l: Line): string {
  const names = l.items.map(itemText).join(", ") + (l.more > 0 ? `, and ${l.more} more` : "");
  const text = [l.count === null ? "" : String(l.count), l.lead, names].filter(Boolean).join(" ");
  return text.endsWith(".") ? text : `${text}.`;
}

/** The result lines as plain text, each ending in a full stop, for the announcer. */
export function dropResultText(
  outcome: DropOutcome,
  entries: SessionEntry[],
  hasFailedEntries: boolean,
): string {
  return buildLines(outcome, entries, hasFailedEntries).map(lineText).join(" ");
}

interface DropResultProps {
  outcome: DropOutcome;
  /** The entries of the same session result as `outcome`. */
  entries: SessionEntry[];
  hasFailedEntries: boolean;
  onDismiss: () => void;
}

function FullPaths({ lines }: { lines: Line[] }) {
  const groups = lines.flatMap((l) => (l.full ? [{ key: l.key, full: l.full }] : []));
  if (groups.length === 0) return null;
  return (
    <details className="drop-result__paths">
      <summary>Full paths</summary>
      {groups.map((l) => (
        <div key={l.key} className="drop-result__group">
          <p className="drop-result__group-lead">{l.full.lead}</p>
          <ul>
            {"paths" in l.full
              ? l.full.paths.map((p, i) => (
                  <li key={i} className="mono">
                    {p}
                  </li>
                ))
              : l.full.entries.map((e, i) => (
                  <li key={i}>
                    <span className={e.mono ? "mono" : undefined}>{e.label}</span>
                    <ul>
                      {e.candidates.map((c, j) => (
                        <li key={j} className="mono">
                          {c}
                        </li>
                      ))}
                    </ul>
                  </li>
                ))}
          </ul>
        </div>
      ))}
    </details>
  );
}

/** What the last drop did. Deliberately not a live region: the view announces `dropResultText`. */
export function DropResult({ outcome, entries, hasFailedEntries, onDismiss }: DropResultProps) {
  const lines = buildLines(outcome, entries, hasFailedEntries);
  return (
    <section className="drop-result" aria-label="Drop result">
      <div className="drop-result__body">
        <ul className="drop-result__lines">
          {lines.map((l) => (
            <li
              key={l.key}
              className={`drop-result__line${l.key === "hint" || l.key === "empty" ? " drop-result__line--hint" : ""}`}
            >
              <span className={`drop-result__icon drop-result__icon--${l.tone}`}>
                <Icon name={l.icon} />
              </span>
              <span>
                {l.count !== null && <span className="num">{l.count} </span>}
                {l.lead}
                {l.items.length > 0 && " "}
                {l.items.map((it, i) => (
                  <span key={i}>
                    <span title={it.title}>
                      {it.mono ? <span className="mono">{it.label}</span> : it.label}
                      {it.note && (
                        <>
                          {" ("}
                          {"count" in it.note ? (
                            <>
                              <span className="num">{it.note.count}</span> files
                            </>
                          ) : (
                            it.note.text
                          )}
                          {")"}
                        </>
                      )}
                    </span>
                    {i < l.items.length - 1 && ", "}
                  </span>
                ))}
                {l.more > 0 && `, and ${l.more} more`}
              </span>
            </li>
          ))}
        </ul>
        <FullPaths lines={lines} />
      </div>
      <Button variant="quiet" icon="x" aria-label="Dismiss drop result" onClick={onDismiss} />
    </section>
  );
}

/** Shown while a drop is being matched. Not focusable, not a live region. */
export function DropPending() {
  return (
    <div className="drop-result drop-result--pending">
      <ul className="drop-result__lines">
        <li className="drop-result__line drop-result__line--hint">
          <span className="drop-result__icon drop-result__icon--muted">
            <Icon name="info" />
          </span>
          <span>Matching dropped files…</span>
        </li>
      </ul>
    </div>
  );
}
