import { Button } from "../../app/Button";
import { Icon, type IconName } from "../../app/icons";
import type { DropOutcome } from "../../lib/generated/DropOutcome";
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
    phrase: () => "can't be assigned: the path has unsupported characters. Rename it",
    icon: "alert-triangle",
    tone: "warn",
  },
};
const ORDER = Object.keys(reasons) as UnmatchedReason[];

const HINT = "Files for entries with errors can't be matched until those errors are fixed.";
const EMPTY = "Nothing was dropped that could be matched.";
const MAX_NAMES = 8;

interface Line {
  key: string;
  icon: IconName;
  tone: Tone;
  /** Leading count, rendered with `.num`; null for lines without one. */
  count: number | null;
  /** Text after the count, up to and including the colon when there are names. */
  lead: string;
  names: { label: string; title: string }[];
  more: number;
  tail: string;
}

/** The last path segment of a display path. */
function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
}

function named(paths: string[]) {
  return {
    names: paths.slice(0, MAX_NAMES).map((p) => ({ label: baseName(p), title: p })),
    more: Math.max(0, paths.length - MAX_NAMES),
  };
}

function buildLines(outcome: DropOutcome, hasFailedEntries: boolean): Line[] {
  const lines: Line[] = [];
  const none = { names: [], more: 0, tail: "" };
  if (outcome.matched.length > 0) {
    lines.push({
      key: "matched",
      icon: "check-circle",
      tone: "ok",
      count: outcome.matched.length,
      lead: "matched",
      ...none,
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
      tail: "",
      ...named(paths),
    });
  }
  const showHint = outcome.unmatched.some((u) => u.reason === "noEntry" || u.reason === "folderNoMatch");
  if (hasFailedEntries && showHint) {
    lines.push({
      key: "hint",
      icon: "info",
      tone: "muted",
      count: null,
      lead: HINT,
      ...none,
    });
  }
  if (outcome.ambiguous.length > 0) {
    const items = outcome.ambiguous.map(
      (a) =>
        `${baseName(a.candidates[0] ?? a.id)} could be ${a.candidates.length} ${one(a.candidates.length, "file", "files")}`,
    );
    lines.push({
      key: "ambiguous",
      icon: "alert-triangle",
      tone: "warn",
      count: outcome.ambiguous.length,
      lead: "ambiguous:",
      names: [],
      more: 0,
      tail: `${items.join(", ")} — use Browse`,
    });
  }
  return lines;
}

function lineText(l: Line): string {
  const names = l.names.map((n) => n.label).join(", ") + (l.more > 0 ? ` and ${l.more} more` : "");
  const text = [l.count === null ? "" : String(l.count), l.lead, names, l.tail].filter(Boolean).join(" ");
  return text.endsWith(".") ? text : `${text}.`;
}

/** The result lines as plain text, each ending in a full stop, for the announcer. */
export function dropResultText(outcome: DropOutcome, hasFailedEntries: boolean): string {
  const lines = buildLines(outcome, hasFailedEntries);
  if (lines.length === 0) return EMPTY;
  return lines.map(lineText).join(" ");
}

interface DropResultProps {
  outcome: DropOutcome;
  hasFailedEntries: boolean;
  onDismiss: () => void;
}

/** What the last drop did. Deliberately not a live region: the view announces `dropResultText`. */
export function DropResult({ outcome, hasFailedEntries, onDismiss }: DropResultProps) {
  const lines = buildLines(outcome, hasFailedEntries);
  return (
    <section className="drop-result" aria-label="Drop result">
      <ul className="drop-result__lines">
        {lines.length === 0 && (
          <li className="drop-result__line drop-result__line--hint">
            <span className="drop-result__icon drop-result__icon--muted">
              <Icon name="info" />
            </span>
            <span>{EMPTY}</span>
          </li>
        )}
        {lines.map((l) => (
          <li
            key={l.key}
            className={`drop-result__line${l.key === "hint" ? " drop-result__line--hint" : ""}`}
          >
            <span className={`drop-result__icon drop-result__icon--${l.tone}`}>
              <Icon name={l.icon} />
            </span>
            <span>
              {l.count !== null && <span className="num">{l.count} </span>}
              {l.lead}
              {l.names.length > 0 && " "}
              {l.names.map((n, i) => (
                <span key={i}>
                  <span title={n.title}>{n.label}</span>
                  {i < l.names.length - 1 && ", "}
                </span>
              ))}
              {l.more > 0 && ` and ${l.more} more`}
              {l.tail && ` ${l.tail}`}
            </span>
          </li>
        ))}
      </ul>
      <Button variant="quiet" icon="x" aria-label="Dismiss drop result" onClick={onDismiss} />
    </section>
  );
}
