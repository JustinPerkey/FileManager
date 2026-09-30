import type { Diagnostic } from "../../lib/generated/Diagnostic";
import type { EntryFailure } from "../../lib/generated/EntryFailure";
import { DiagnosticList } from "./DiagnosticList";
import { FailureList } from "./FailureList";

interface BuildReportProps {
  leftOut: EntryFailure[];
  manifestErrors: Diagnostic[];
  warnings: Diagnostic[];
}

export function BuildReport({ leftOut, manifestErrors, warnings }: BuildReportProps) {
  if (leftOut.length === 0 && manifestErrors.length === 0 && warnings.length === 0) return null;
  return (
    <div
      className="build-report"
      role="region"
      aria-label="Build report"
      // The region may scroll, so it is the one tab stop that makes it keyboard scrollable.
      // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
      tabIndex={0}
    >
      {leftOut.length > 0 && (
        <section>
          <h3 className="build-report__group">
            Left out of the archive (<span className="num">{leftOut.length}</span>)
          </h3>
          <FailureList failures={leftOut} headingLevel={4} />
        </section>
      )}
      {manifestErrors.length > 0 && (
        <section>
          <h3 className="build-report__group">
            Manifest errors (<span className="num">{manifestErrors.length}</span>)
          </h3>
          <DiagnosticList diagnostics={manifestErrors} label="Manifest errors" />
        </section>
      )}
      {warnings.length > 0 && (
        <details className="build-report__warnings">
          <summary>
            <h3 className="build-report__group">
              Warnings (<span className="num">{warnings.length}</span>)
            </h3>
          </summary>
          <DiagnosticList diagnostics={warnings} label="Warnings" />
        </details>
      )}
    </div>
  );
}
