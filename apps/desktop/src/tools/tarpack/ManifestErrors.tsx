import { useId, type RefObject } from "react";
import { useDevMode } from "../../app/devMode";
import { Button } from "../../app/Button";
import { Icon } from "../../app/icons";
import type { SessionManifest } from "../../lib/generated/SessionManifest";
import { DiagnosticList } from "./DiagnosticList";
import { FailureList } from "./FailureList";

interface ManifestErrorsProps {
  manifest: SessionManifest;
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
  onEdit: () => void;
  reportRef: RefObject<HTMLDivElement | null>;
}

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

function consequence(m: SessionManifest): string {
  if (m.entriesWithheld)
    return "No files can be listed or built until the manifest errors are fixed.";
  const n = m.failedEntries.length;
  if (n > 0)
    return n === 1
      ? "1 file is left out of the list and the archive until it's fixed."
      : `${n} files are left out of the list and the archive until they're fixed.`;
  return "Every file is listed and can still be built.";
}

export function ManifestErrors({
  manifest,
  expanded,
  onExpandedChange,
  onEdit,
  reportRef,
}: ManifestErrorsProps) {
  const headingId = useId();
  const devMode = useDevMode();
  const hasErrors = manifest.errorCount > 0;
  const warnings = manifest.warnings;
  if (!hasErrors && warnings.length === 0) return null;

  return (
    <>
      {hasErrors && (
        <div className="error-region">
          <div className="error-region__notice">
            <span className="error-region__icon">
              <Icon name="x-circle" />
            </span>
            <div className="error-region__text">
              <h2 id={headingId} className="error-region__heading">
                <span className="num">{manifest.errorCount}</span>{" "}
                {plural(manifest.errorCount, "error", "errors")} in this manifest
              </h2>
              <p className="error-region__consequence">{consequence(manifest)}</p>
            </div>
            <div className="error-region__actions">
              <Button
                icon="chevron-down"
                iconAfter
                iconFlip={expanded}
                aria-expanded={expanded}
                aria-controls="manifest-error-report"
                onClick={() => onExpandedChange(!expanded)}
              >
                {expanded ? "Hide errors" : "Show errors"}
              </Button>
              {devMode && <Button onClick={onEdit}>Edit in editor</Button>}
            </div>
          </div>
          <div
            id="manifest-error-report"
            className="error-region__report"
            role="region"
            aria-labelledby={headingId}
            // The body may scroll, so it is the one tab stop that makes it keyboard scrollable.
            // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
            tabIndex={0}
            hidden={!expanded}
            ref={reportRef}
          >
            {manifest.errors.length > 0 && (
              <section>
                <h3 className="error-region__group">Whole manifest</h3>
                <DiagnosticList diagnostics={manifest.errors} label="Whole manifest errors" />
              </section>
            )}
            {manifest.failedEntries.length > 0 && (
              <section>
                <h3 className="error-region__group">Files with errors</h3>
                <FailureList failures={manifest.failedEntries} headingLevel={4} />
              </section>
            )}
          </div>
        </div>
      )}
      {warnings.length > 0 && (
        <details className="warnings">
          <summary className="warnings__summary">
            <span className="warnings__icon">
              <Icon name="alert-triangle" />
            </span>
            <span className="num">{warnings.length}</span> {plural(warnings.length, "warning", "warnings")}
          </summary>
          <DiagnosticList diagnostics={warnings} label="Warnings" />
        </details>
      )}
    </>
  );
}
