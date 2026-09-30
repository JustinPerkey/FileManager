import type { EntryFailure } from "../../lib/generated/EntryFailure";
import { DiagnosticList } from "./DiagnosticList";

interface FailureListProps {
  failures: EntryFailure[];
  headingLevel: 3 | 4;
}

export function FailureList({ failures, headingLevel }: FailureListProps) {
  if (failures.length === 0) return null;
  const Heading = `h${headingLevel}` as "h3" | "h4";
  return (
    <ul className="failure-list">
      {failures.map((f) => {
        const name = f.id ?? `Entry #${f.index}`;
        return (
          <li key={f.index} className="failure-list__item">
            <Heading className="failure-list__name">
              {f.id !== null ? (
                <span className="mono">{f.id}</span>
              ) : (
                <>
                  Entry #<span className="num">{f.index}</span>
                </>
              )}
            </Heading>
            <p className="failure-list__where">
              {f.source !== null && (
                <>
                  source <span className="mono">{f.source}</span>
                  <span aria-hidden="true"> · </span>
                </>
              )}
              <span className="mono">[[file]]</span> on line <span className="num">{f.line}</span>
            </p>
            <DiagnosticList diagnostics={f.errors} label={`Errors in ${name}`} />
          </li>
        );
      })}
    </ul>
  );
}
