import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from "react";
import { Banner } from "../../app/Banner";
import { Button } from "../../app/Button";
import { ConfirmDialog } from "../../app/ConfirmDialog";
import type { ApplySummary } from "../../lib/generated/ApplySummary";
import type { ScheduleError } from "../../lib/generated/ScheduleError";
import type { SchedulePreview } from "../../lib/generated/SchedulePreview";
import type { ScheduleSession } from "../../lib/generated/ScheduleSession";
import { apply, openText, openXml, reloadText, session as fetchSession } from "../../lib/schedule";
import { openFileDialog } from "../../lib/tauri";
import { errorMessage, fileName, toScheduleError, type ViewError } from "./errorMessages";

const TEXT = [
  { name: "Text", extensions: ["txt"] },
  { name: "All files", extensions: ["*"] },
];
const XML = [
  { name: "XML", extensions: ["xml"] },
  { name: "All files", extensions: ["*"] },
];
const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

/** What to announce after the text file is read. A failed parse needs none: its banner is an alert. */
function parsedSentence(s: ScheduleSession): string | null {
  if (s.parseError) return s.parseError.kind === "ParseFailed" ? null : errorMessage(s.parseError);
  if (s.preview) {
    const n = s.preview.rows.length;
    return `Schedule read: ${n} ${plural(n, "entry", "entries")}.`;
  }
  return null;
}

function applyReason(s: ScheduleSession): string | null {
  if (!s.textPath) return "Choose a schedule text file.";
  if (!s.preview) return "The schedule file must be read without errors first.";
  if (s.preview.rows.length === 0) return "The schedule file has no entries to add.";
  if (!s.xmlPath) return "Choose the XML file to update.";
  return null;
}

function resultSentence(r: ApplySummary): string {
  return `Added ${r.added} ${plural(r.added, "entry", "entries")} to ${fileName(r.xmlPath)}.`;
}

export function ScheduleView() {
  const [session, setSession] = useState<ScheduleSession | null>(null);
  const [restoreFailed, setRestoreFailed] = useState(false);
  const [commandError, setCommandError] = useState<ViewError | null>(null);
  const [failureCount, setFailureCount] = useState(0);
  const [confirming, setConfirming] = useState(false);
  const [applying, setApplying] = useState(false);
  const [result, setResult] = useState<ApplySummary | null>(null);
  // The text and XML of the last successful apply: adding them again would duplicate entries.
  const [applied, setApplied] = useState<string | null>(null);
  const [loadAttempt, setLoadAttempt] = useState(0);
  // After the confirm dialog closes, the Add button is disabled (busy), so the
  // dialog cannot return focus to it; focus moves here once applying ends.
  const refocus = useRef(false);
  const addRef = useRef<HTMLButtonElement>(null);
  const outcomeRef = useRef<HTMLDivElement>(null);
  const errorRef = useRef<HTMLDivElement>(null);
  const [announcement, setAnnouncement] = useState("");
  const frame = useRef(0);
  const reasonId = useId();

  const announce = useCallback((text: string) => {
    cancelAnimationFrame(frame.current);
    setAnnouncement("");
    frame.current = requestAnimationFrame(() => setAnnouncement(text));
  }, []);
  useEffect(() => () => cancelAnimationFrame(frame.current), []);

  // The error banner is an alert, so failures are not also announced.
  const fail = useCallback((e: unknown) => {
    setCommandError(toScheduleError(e));
    setFailureCount((n) => n + 1);
  }, []);

  useEffect(() => {
    let live = true;
    fetchSession().then(
      (s) => {
        if (!live) return;
        setSession(s);
        setCommandError(null);
      },
      (e) => live && (setRestoreFailed(true), fail(e)),
    );
    return () => {
      live = false;
    };
  }, [fail, loadAttempt]);

  useEffect(() => {
    if (applying || !refocus.current) return;
    refocus.current = false;
    const outcome =
      outcomeRef.current?.querySelector<HTMLElement>(".banner") ??
      errorRef.current?.querySelector<HTMLElement>(".banner");
    if (outcome) {
      outcome.tabIndex = -1;
      outcome.focus();
    } else addRef.current?.focus();
  }, [applying]);

  /** Runs a session command. `parsed` announces the parse outcome (opening or reloading the text). */
  const run = useCallback(
    async (fn: () => Promise<ScheduleSession>, parsed: boolean) => {
      try {
        const next = await fn();
        setSession(next);
        setCommandError(null);
        setResult(null);
        setApplied(null);
        const sentence = parsed ? parsedSentence(next) : null;
        if (sentence) announce(sentence);
      } catch (e) {
        fail(e);
      }
    },
    [announce, fail],
  );

  const onChooseText = useCallback(async () => {
    try {
      const path = await openFileDialog({ filters: TEXT });
      if (path) await run(() => openText(path), true);
    } catch (e) {
      fail(e);
    }
  }, [run, fail]);
  const onReload = useCallback(() => run(reloadText, true), [run]);
  const onChooseXml = useCallback(async () => {
    try {
      const path = await openFileDialog({ filters: XML });
      if (path) await run(() => openXml(path), false);
    } catch (e) {
      fail(e);
    }
  }, [run, fail]);

  const applyKey = session ? `${session.textPath ?? ""}\n${session.xmlPath ?? ""}` : null;
  const onApply = useCallback(async () => {
    refocus.current = true;
    setConfirming(false);
    setApplying(true);
    setResult(null);
    setCommandError(null);
    try {
      const summary = await apply();
      setResult(summary);
      setApplied(applyKey);
      announce(resultSentence(summary));
    } catch (e) {
      fail(e);
    } finally {
      setApplying(false);
    }
  }, [announce, fail, applyKey]);

  const loading = session === null && !restoreFailed;
  const reason = session ? applyReason(session) : null;
  const xmlName = fileName(session?.xmlPath ?? "");

  return (
    <section className="tool-view schedule" aria-busy={loading ? true : undefined}>
      <div className="visually-hidden" role="status" aria-live="polite">
        {announcement}
      </div>
      <header className="schedule__header">
        <h1>Schedule Creator</h1>
        <p className="schedule__intro">
          Read a schedule from a text file and add it to an existing XML file.
        </p>
      </header>
      {loading && <span className="visually-hidden">Loading</span>}
      {restoreFailed && (
        <div>
          <Button
            onClick={() => {
              setRestoreFailed(false);
              setLoadAttempt((n) => n + 1);
            }}
          >
            Try again
          </Button>
        </div>
      )}
      <div ref={errorRef}>
        {commandError && (
          <Banner
            key={failureCount}
            tone="error"
            message={errorMessage(commandError)}
            onDismiss={() => setCommandError(null)}
          >
            <details className="banner__details">
              <summary>Details</summary>
              <p className="mono">{commandError.message}</p>
            </details>
          </Banner>
        )}
      </div>
      {session && (
        <fieldset className="schedule__steps" disabled={applying}>
          <legend className="visually-hidden">Files</legend>
          <FileStep
            step={1}
            title="Schedule text file"
            path={session.textPath}
            chooseLabel="Choose schedule file…"
            onChoose={onChooseText}
            onReload={session.textPath ? onReload : undefined}
          >
            {session.parseError && <ParseProblem error={session.parseError} />}
            {session.preview && <PreviewTable preview={session.preview} />}
          </FileStep>
          <FileStep
            step={2}
            title="XML file to update"
            path={session.xmlPath}
            chooseLabel="Choose XML file…"
            onChoose={onChooseXml}
          />
        </fieldset>
      )}
      {session && (
        <div className="schedule__apply">
          <Button
            ref={addRef}
            variant="primary"
            busy={applying}
            disabled={!session.canApply}
            aria-describedby={reason ? reasonId : undefined}
            onClick={() => setConfirming(true)}
          >
            {applying ? "Adding…" : "Add to XML…"}
          </Button>
          {reason && (
            <p id={reasonId} className="schedule__reason">
              {reason}
            </p>
          )}
        </div>
      )}
      <div ref={outcomeRef}>
        {result && (
          <Banner tone="info" message={resultSentence(result)} onDismiss={() => setResult(null)}>
            <p className="schedule__backup">
              The previous version is saved as <span className="mono">{result.backupPath}</span>
            </p>
          </Banner>
        )}
      </div>
      <ConfirmDialog
        open={confirming}
        title={`Update ${xmlName}?`}
        body={
          <>
            The schedule will be added to <span className="mono">{session?.xmlPath ?? ""}</span>. A backup
            copy of the current file is saved next to it first.
            {applied !== null && applied === applyKey && (
              <>
                {" "}
                <strong>This schedule was already added to this file.</strong> Adding it again may duplicate
                its entries.
              </>
            )}
          </>
        }
        confirmLabel="Update"
        onCancel={() => setConfirming(false)}
        onConfirm={() => void onApply()}
      />
    </section>
  );
}

interface FileStepProps {
  step: number;
  title: string;
  path: string | null;
  chooseLabel: string;
  onChoose: () => void;
  onReload?: () => void;
  children?: ReactNode;
}

function FileStep({ step, title, path, chooseLabel, onChoose, onReload, children }: FileStepProps) {
  const headingId = useId();
  return (
    <section className="schedule__step" aria-labelledby={headingId}>
      <h2 id={headingId} className="schedule__step-title">
        <span className="schedule__step-number" aria-hidden="true">
          {step}
        </span>
        {title}
      </h2>
      <div className="schedule__file">
        {path ? (
          <p className="schedule__path mono" title={path}>
            {path}
          </p>
        ) : (
          <p className="schedule__path schedule__path--none">None chosen</p>
        )}
        <div className="schedule__file-actions">
          <Button onClick={onChoose}>{chooseLabel}</Button>
          {onReload && <Button onClick={onReload}>Reload</Button>}
        </div>
      </div>
      {children}
    </section>
  );
}

function ParseProblem({ error }: { error: ScheduleError }) {
  const notImplemented = error.kind === "ParseNotImplemented";
  return <Banner tone={notImplemented ? "warn" : "error"} message={errorMessage(error)} />;
}

function PreviewTable({ preview }: { preview: SchedulePreview }) {
  const n = preview.rows.length;
  if (n === 0) return <p className="schedule__empty">The schedule file has no entries.</p>;
  return (
    <div className="schedule__preview">
      <table className="schedule__table">
        <caption className="schedule__caption">
          {n} {plural(n, "entry", "entries")} to add
        </caption>
        <thead>
          <tr>
            {preview.columns.map((c, i) => (
              <th key={i} scope="col">
                {c}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {preview.rows.map((row, r) => (
            <tr key={r}>
              {row.map((cell, c) => (
                <td key={c}>{cell}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
