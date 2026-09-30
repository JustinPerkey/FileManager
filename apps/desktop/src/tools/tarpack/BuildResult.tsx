import { useEffect, useRef, useState } from "react";
import { Button } from "../../app/Button";
import { Icon } from "../../app/icons";
import type { BuildSummary } from "../../lib/generated/BuildSummary";
import type { SessionEntry } from "../../lib/generated/SessionEntry";
import type { TarpackError } from "../../lib/generated/TarpackError";
import { BuildReport } from "./BuildReport";
import { errorMessage } from "./errorMessages";
import { FORMAT_NAME } from "./FormatPicker";
import { reportText, resultHeading } from "./reportText";

export type BuildOutcome = { ok: BuildSummary } | { err: TarpackError };

interface BuildResultProps {
  result: BuildOutcome;
  entries: SessionEntry[];
  onReveal: () => void;
  onDismiss: () => void;
}

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

function size(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

function CopyButton({ label, text, visible }: { label: string; text: string; visible?: string }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      return;
    }
    setCopied(true);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setCopied(false), 2000);
  }
  return (
    <span className="build-result__copy">
      <Button
        variant="quiet"
        icon="copy"
        aria-label={visible ? undefined : label}
        onClick={() => void copy()}
      >
        {visible ?? "Copy"}
      </Button>
      <span className="build-result__copied" role="status" aria-live="polite">
        {copied ? "Copied" : ""}
      </span>
    </span>
  );
}

export function BuildResult({ result, entries, onReveal, onDismiss }: BuildResultProps) {
  if ("err" in result) {
    const { err } = result;
    return (
      <section className="build-result build-result--error" aria-labelledby="build-result-heading">
        <div className="build-result__head">
          <span className="build-result__icon build-result__icon--danger">
            <Icon name="x-circle" />
          </span>
          <h2 id="build-result-heading" className="build-result__heading">
            {errorMessage(err, entries)}
          </h2>
          <Button variant="quiet" icon="x" aria-label="Dismiss result" onClick={onDismiss} />
        </div>
        <details className="banner__details">
          <summary>Details</summary>
          <p className="mono">{err.message}</p>
        </details>
      </section>
    );
  }

  const s = result.ok;
  const clean = s.errorCount === 0;
  const sourceOf = (id: string) => entries.find((e) => e.id === id)?.source ?? id;
  return (
    <section className="build-result" aria-labelledby="build-result-heading">
      <div className="build-result__head">
        <span
          className={`build-result__icon ${clean ? "build-result__icon--ok" : "build-result__icon--warn"}`}
        >
          <Icon name={clean ? "check-circle" : "alert-triangle"} />
        </span>
        <h2 id="build-result-heading" className="build-result__heading">
          {resultHeading(s)}
        </h2>
        <Button variant="quiet" icon="x" aria-label="Dismiss result" onClick={onDismiss} />
      </div>
      {!clean && s.leftOut.length > 0 && (
        <p className="build-result__lede">
          The archive holds <span className="num">{s.builtIds.length}</span> of the manifest&apos;s{" "}
          <span className="num">{s.builtIds.length + s.leftOut.length}</span> files. The rest have errors,
          listed below.
        </p>
      )}
      <p className="build-result__path mono" title={s.path}>
        {s.path}
      </p>
      <p className="build-result__facts">
        {FORMAT_NAME[s.format]} · <span className="num">{s.files}</span> {plural(s.files, "file", "files")},{" "}
        <span className="num">{s.dirs}</span> {plural(s.dirs, "folder", "folders")} ·{" "}
        <span className="num">{size(s.bytes)}</span>
        {s.format !== "tar" && (
          <>
            {" "}
            (<span className="num">{size(s.uncompressedBytes)}</span> uncompressed)
          </>
        )}
      </p>
      <div className="build-result__block">
        <h3 className="build-result__label">SHA-256</h3>
        <div className="build-result__row">
          <p className="build-result__code mono">{s.sha256Hex}</p>
          <CopyButton label="Copy SHA-256" text={s.sha256Hex} />
        </div>
      </div>
      <div className="build-result__block">
        <h3 className="build-result__label">Extract on the target</h3>
        <div className="build-result__row">
          <p className="build-result__code build-result__code--block mono">{s.extractCommand}</p>
          <CopyButton label="Copy extraction command" text={s.extractCommand} />
        </div>
        <p className="build-result__hint">
          Run this in the folder that holds the file. -P keeps the absolute paths.
        </p>
      </div>
      {s.normalizedEntries.length > 0 && (
        <div className="build-result__block">
          <h3 className="build-result__label">Line endings converted</h3>
          <ul className="build-result__list">
            {s.normalizedEntries.map((n) => (
              <li key={n.id}>
                <span className="mono">{sourceOf(n.id)}</span>:{" "}
                {n.crlfReplaced === 0 ? (
                  "no CRLF found"
                ) : (
                  <>
                    <span className="num">{n.crlfReplaced}</span> CRLF replaced
                  </>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
      <BuildReport leftOut={s.leftOut} manifestErrors={s.manifestErrors} warnings={s.warnings} />
      <div className="build-result__actions">
        <Button icon="folder" onClick={onReveal}>
          Show in folder
        </Button>
        {!clean && <CopyButton label="Copy report" visible="Copy report" text={reportText(s)} />}
      </div>
    </section>
  );
}
