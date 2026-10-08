import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Banner } from "../../app/Banner";
import { Button } from "../../app/Button";
import { ConfirmDialog } from "../../app/ConfirmDialog";
import { DropZone } from "../../app/DropZone";
import type { ApplySummary } from "../../lib/generated/ApplySummary";
import type { FileSlot } from "../../lib/generated/FileSlot";
import type { ScheduleSession } from "../../lib/generated/ScheduleSession";
import { apply, session as fetchSession, setDropped, setText, setXml } from "../../lib/schedule";
import { openFileDialog, type DialogFilter } from "../../lib/tauri";
import { errorMessage, fileName, toScheduleError, type ViewError } from "./errorMessages";

const TEXT: DialogFilter[] = [
  { name: "Text", extensions: ["txt"] },
  { name: "All files", extensions: ["*"] },
];
const XML: DialogFilter[] = [
  { name: "XML", extensions: ["xml"] },
  { name: "All files", extensions: ["*"] },
];
const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

/** A typed or pasted path: trimmed, without the quotes Explorer's "Copy as path" adds. Empty means none. */
export function cleanPath(typed: string): string | null {
  const t = typed
    .trim()
    .replace(/^"(.*)"$/, "$1")
    .trim();
  return t === "" ? null : t;
}

function applyReason(s: ScheduleSession): string | null {
  if (!s.text) return "Choose a schedule file.";
  if (!s.xml) return "Choose the XML file to update.";
  if (s.text.error || s.xml.error) return "Fix the file paths above first.";
  return null;
}

function resultSentence(r: ApplySummary): string {
  return `Added ${r.added} ${plural(r.added, "entry", "entries")} to ${fileName(r.xmlPath)}.`;
}

function dropSentence(s: ScheduleSession): string {
  const parts: string[] = [];
  if (s.text) parts.push(`Schedule file: ${fileName(s.text.path)}${s.text.error ? " (has a problem)" : ""}.`);
  if (s.xml) parts.push(`XML file: ${fileName(s.xml.path)}${s.xml.error ? " (has a problem)" : ""}.`);
  return parts.join(" ");
}

export function ScheduleView() {
  const [session, setSession] = useState<ScheduleSession | null>(null);
  const [restoreFailed, setRestoreFailed] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [commandError, setCommandError] = useState<ViewError | null>(null);
  const [failureCount, setFailureCount] = useState(0);
  const [confirming, setConfirming] = useState(false);
  const [applying, setApplying] = useState(false);
  const [result, setResult] = useState<ApplySummary | null>(null);
  // The files of the last successful apply: adding the same schedule again would duplicate entries.
  const [applied, setApplied] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState("");
  const frame = useRef(0);
  // After the confirm dialog closes, the Add button is disabled (busy), so the
  // dialog cannot return focus to it; focus moves here once applying ends.
  const refocus = useRef(false);
  const addRef = useRef<HTMLButtonElement>(null);
  const outcomeRef = useRef<HTMLDivElement>(null);
  const errorRef = useRef<HTMLDivElement>(null);
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

  const run = useCallback(
    async (fn: () => Promise<ScheduleSession>): Promise<ScheduleSession | null> => {
      try {
        const next = await fn();
        setSession(next);
        setCommandError(null);
        setResult(null);
        return next;
      } catch (e) {
        fail(e);
        return null;
      }
    },
    [fail],
  );

  const chooseText = useCallback(async () => {
    try {
      const path = await openFileDialog({ filters: TEXT });
      if (path) await run(() => setText(path));
    } catch (e) {
      fail(e);
    }
  }, [run, fail]);
  const chooseXml = useCallback(async () => {
    try {
      const path = await openFileDialog({ filters: XML });
      if (path) await run(() => setXml(path));
    } catch (e) {
      fail(e);
    }
  }, [run, fail]);
  const typeText = useCallback((path: string | null) => void run(() => setText(path)), [run]);
  const typeXml = useCallback((path: string | null) => void run(() => setXml(path)), [run]);
  const onDrop = useCallback(
    async (paths: string[]) => {
      const next = await run(() => setDropped(paths));
      if (next) announce(dropSentence(next));
    },
    [run, announce],
  );

  const applyKey = session ? `${session.text?.path ?? ""}\n${session.xml?.path ?? ""}` : null;
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

  return (
    <section className="tool-view schedule" aria-busy={loading ? true : undefined}>
      <div className="visually-hidden" role="status" aria-live="polite">
        {announcement}
      </div>
      <DropZone
        enabled={!!session && !applying}
        disabledReason={applying ? "The XML file is being updated" : "Not ready yet"}
        label="Drop a schedule file, an XML file, or both"
        onDrop={(paths) => void onDrop(paths)}
      />
      <header className="schedule__header">
        <h1>Schedule Creator</h1>
        <p className="schedule__intro">
          Choose a schedule text file and the XML file to add it to. Type or paste a path, browse for it, or
          drop the files on the window.
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
          <FileField
            step={1}
            label="Schedule file"
            placeholder="C:\path\to\schedule.txt"
            slot={session.text}
            browseLabel="Browse for the schedule file"
            onBrowse={chooseText}
            onCommit={typeText}
          />
          <FileField
            step={2}
            label="XML file to update"
            placeholder="C:\path\to\schedule.xml"
            slot={session.xml}
            browseLabel="Browse for the XML file"
            onBrowse={chooseXml}
            onCommit={typeXml}
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
        title={`Update ${fileName(session?.xml?.path ?? "")}?`}
        body={
          <>
            The schedule in <span className="mono">{session?.text?.path ?? ""}</span> will be read and added
            to <span className="mono">{session?.xml?.path ?? ""}</span>. A backup copy of the XML file is
            saved next to it first.
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

interface FileFieldProps {
  step: number;
  label: string;
  placeholder: string;
  slot: FileSlot | null;
  browseLabel: string;
  onBrowse: () => void;
  /** A typed path, cleaned; `null` clears the file. Called on Enter or leaving the field, when changed. */
  onCommit: (path: string | null) => void;
}

function FileField({ step, label, placeholder, slot, browseLabel, onBrowse, onCommit }: FileFieldProps) {
  const inputId = useId();
  const errorId = useId();
  const current = slot?.path ?? "";
  const [draft, setDraft] = useState(current);
  // Follow the session when it changes (browse, drop): adjust state while rendering.
  const [shown, setShown] = useState(current);
  if (shown !== current) {
    setShown(current);
    setDraft(current);
  }

  const commit = () => {
    const path = cleanPath(draft);
    if ((path ?? "") === current) {
      setDraft(current);
      return;
    }
    onCommit(path);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      commit();
    } else if (e.key === "Escape" && draft !== current) {
      e.preventDefault();
      setDraft(current);
    }
  };

  return (
    <div className="schedule__step">
      <label htmlFor={inputId} className="schedule__step-title">
        <span className="schedule__step-number" aria-hidden="true">
          {step}
        </span>
        {label}
      </label>
      <div className="schedule__file">
        <input
          id={inputId}
          className="schedule__input mono"
          type="text"
          spellCheck={false}
          autoComplete="off"
          placeholder={placeholder}
          value={draft}
          aria-invalid={slot?.error ? true : undefined}
          aria-describedby={slot?.error ? errorId : undefined}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={onKeyDown}
        />
        <Button onClick={onBrowse} aria-label={browseLabel}>
          Browse…
        </Button>
      </div>
      {slot?.error && (
        <p id={errorId} className="schedule__field-error">
          {errorMessage(slot.error)}
        </p>
      )}
    </div>
  );
}
