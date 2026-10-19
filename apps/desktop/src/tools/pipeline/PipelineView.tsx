import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Banner } from "../../app/Banner";
import { Button } from "../../app/Button";
import { ConfirmDialog } from "../../app/ConfirmDialog";
import { Icon, type IconName } from "../../app/icons";
import type { DirectorySlot } from "../../lib/generated/DirectorySlot";
import type { PipelineSession } from "../../lib/generated/PipelineSession";
import type { StepId } from "../../lib/generated/StepId";
import type { StepView } from "../../lib/generated/StepView";
import { runStep, session as fetchSession, setDirectory } from "../../lib/pipeline";
import { openDirectoryDialog } from "../../lib/tauri";
import { errorMessage, toPipelineError, type ViewError } from "./errorMessages";
import { stepCopy } from "./steps";

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

/** A typed or pasted path: trimmed, without the quotes Explorer's "Copy as path" adds. Empty means none. */
export function cleanPath(typed: string): string | null {
  const t = typed
    .trim()
    .replace(/^"(.*)"$/, "$1")
    .trim();
  return t === "" ? null : t;
}

type Status = {
  label: string;
  icon: IconName;
  tone: "muted" | "ok" | "warn" | "error";
};

export function stepStatus(view: StepView, running: boolean): Status {
  if (running) return { label: "Running…", icon: "circle", tone: "muted" };
  if (view.error?.kind === "StepNotImplemented")
    return { label: "Not implemented", icon: "info", tone: "warn" };
  if (view.error) return { label: "Failed", icon: "x-circle", tone: "error" };
  if (view.report?.passed) return { label: "Passed", icon: "check-circle", tone: "ok" };
  if (view.report) {
    const n = view.report.issues.length;
    return {
      label: `${n} ${plural(n, "problem", "problems")}`,
      icon: "alert-triangle",
      tone: "error",
    };
  }
  if (!view.unlocked) return { label: "Waiting", icon: "circle", tone: "muted" };
  return { label: "Not run", icon: "circle", tone: "muted" };
}

const passed = (s: PipelineSession, id: StepId) => !!s.steps.find((v) => v.id === id)?.report?.passed;

function runReason(s: PipelineSession, running: boolean): string | null {
  if (running) return "A step is running.";
  if (!s.directory) return "Choose a folder.";
  if (s.directory.error) return "Fix the folder path above first.";
  return null;
}

/** What a confirmation is for: one step, or every step in order. */
type Pending = { kind: "step"; id: StepId; viaNext?: boolean } | { kind: "all" };

export function PipelineView() {
  const [session, setSession] = useState<PipelineSession | null>(null);
  const [restoreFailed, setRestoreFailed] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [commandError, setCommandError] = useState<ViewError | null>(null);
  const [failureCount, setFailureCount] = useState(0);
  // The step this view started and is waiting for. A step started before the
  // view was last mounted shows only as `session.running`.
  const [active, setActive] = useState<StepId | null>(null);
  const [runningAll, setRunningAll] = useState(false);
  const [pending, setPending] = useState<Pending | null>(null);
  // The folder named in the open confirmation: exactly this one is used.
  const [confirmedDir, setConfirmedDir] = useState("");
  const [announcement, setAnnouncement] = useState("");
  const frame = useRef(0);
  const stepRefs = useRef(new Map<StepId, HTMLElement>());
  const nextRef = useRef<HTMLButtonElement>(null);
  // The step "Run next" started, so only that button shows busy for it.
  const [viaNext, setViaNext] = useState<StepId | null>(null);
  // After a "Run next" step, focus returns to "Run next" once it is enabled
  // again, so keyboard users can keep walking through the steps.
  const refocusNext = useRef(false);
  const reasonId = useId();
  const allPassedId = useId();
  // Run all stops starting steps once the view is gone.
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const announce = useCallback((text: string) => {
    cancelAnimationFrame(frame.current);
    setAnnouncement("");
    frame.current = requestAnimationFrame(() => setAnnouncement(text));
  }, []);
  useEffect(() => () => cancelAnimationFrame(frame.current), []);

  // The error banner is an alert, so failures are not also announced.
  const fail = useCallback((e: unknown) => {
    setCommandError(toPipelineError(e));
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

  // A step started before this view mounted (the user left and came back):
  // nothing here is awaiting it, so poll until the backend says it is done.
  // A failed poll is reported once and retried; the counter re-runs the effect.
  const orphan = active === null && !!session?.running;
  const [pollFailures, setPollFailures] = useState(0);
  useEffect(() => {
    if (!orphan) return;
    let live = true;
    const timer = setTimeout(() => {
      fetchSession().then(
        (s) => {
          if (!live) return;
          setSession(s);
          setPollFailures(0);
        },
        (e) => {
          if (!live) return;
          if (pollFailures === 0) fail(e);
          setPollFailures((n) => n + 1);
        },
      );
    }, 1000);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [orphan, session, fail, pollFailures]);

  // Only the newest folder change is shown: an older response arriving late
  // would show a folder the backend no longer holds.
  const latest = useRef(0);
  const choose = useCallback(
    async (path: string | null) => {
      const id = ++latest.current;
      try {
        const next = await setDirectory(path);
        if (id !== latest.current) return;
        setSession(next);
        setCommandError(null);
        // A field already holding focus does not announce its new error, so say it.
        if (next.directory?.error) announce(errorMessage(next.directory.error));
      } catch (e) {
        if (id === latest.current) fail(e);
      }
    },
    [announce, fail],
  );
  const dirPath = session?.directory?.path;
  const browse = useCallback(async () => {
    try {
      const path = await openDirectoryDialog({ defaultPath: dirPath });
      if (path) await choose(path);
    } catch (e) {
      fail(e);
    }
  }, [choose, fail, dirPath]);

  /** Runs one step. Resolves to the new session, or `null` when the step could not start. */
  const runOne = useCallback(
    async (id: StepId, dir: string): Promise<PipelineSession | null> => {
      setActive(id);
      try {
        const next = await runStep(id, dir);
        setSession(next);
        setCommandError(null);
        return next;
      } catch (e) {
        fail(e);
        return null;
      } finally {
        setActive(null);
      }
    },
    [fail],
  );

  const focusStep = (id: StepId) => stepRefs.current.get(id)?.focus();
  // The clicked Run button is disabled while its step runs, so focus moves to
  // the step instead of falling to the page. An effect, so it runs after the
  // confirmation dialog has returned focus to its opener.
  useEffect(() => {
    if (active) stepRefs.current.get(active)?.focus();
  }, [active]);

  const runSingle = useCallback(
    async (id: StepId, dir: string, fromNext: boolean) => {
      if (fromNext) setViaNext(id);
      const next = await runOne(id, dir);
      setViaNext(null);
      if (!next) return;
      const view = next.steps.find((v) => v.id === id);
      if (view) announce(`${stepCopy[id].title}: ${stepStatus(view, false).label}.`);
      if (fromNext && next.nextStep !== null && next.canRun) refocusNext.current = true;
      else focusStep(id);
    },
    [runOne, announce],
  );

  /** Every step in order, stopping at the first that does not pass. */
  const runAll = useCallback(
    async (dir: string) => {
      if (!session) return;
      setRunningAll(true);
      let last: StepId | null = null;
      let current: PipelineSession | null = session;
      for (const { id } of session.steps) {
        if (!mounted.current) return;
        last = id;
        current = await runOne(id, dir);
        if (!current || !passed(current, id)) break;
      }
      if (!mounted.current) return;
      setRunningAll(false);
      if (current && last) {
        const all = current.steps.every((v) => v.report?.passed);
        announce(all ? "All steps passed." : `Stopped at ${stepCopy[last].title}.`);
        focusStep(last);
      }
    },
    [session, runOne, announce],
  );

  const start = (p: Pending) => {
    // A refocus left over from an earlier run must not fire on this one.
    refocusNext.current = false;
    if (!session?.directory) return;
    const dir = session.directory.path;
    const needsConfirm =
      p.kind === "all" ? session.steps.some((v) => stepCopy[v.id].confirm) : stepCopy[p.id].confirm;
    if (needsConfirm) {
      setConfirmedDir(dir);
      setPending(p);
    } else if (p.kind === "all") void runAll(dir);
    else void runSingle(p.id, dir, !!p.viaNext);
  };
  const onConfirm = () => {
    const p = pending;
    setPending(null);
    if (!p) return;
    if (p.kind === "all") void runAll(confirmedDir);
    else void runSingle(p.id, confirmedDir, !!p.viaNext);
  };

  const running = active ?? session?.running ?? null;
  const busy = running !== null || runningAll;
  const loading = session === null && !restoreFailed;
  const reason = session ? runReason(session, busy) : null;
  const canRun = !!session?.canRun && !busy;
  const next = session?.nextStep ?? null;
  const allPassed = !!session && next === null;
  useEffect(() => {
    if (refocusNext.current && canRun && next !== null) {
      refocusNext.current = false;
      nextRef.current?.focus();
    }
  }, [canRun, next]);

  return (
    <section className="tool-view pipeline" aria-busy={loading ? true : undefined}>
      <div className="visually-hidden" role="status" aria-live="polite">
        {announcement}
      </div>
      <header className="pipeline__header">
        <h1>Pipeline</h1>
        <p className="pipeline__intro">
          Choose a folder. Each subfolder is processed by the steps below, in order. Run one step at a time,
          or run them all.
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
      {session && (
        <>
          <DirectoryField slot={session.directory} disabled={busy} onBrowse={browse} onCommit={choose} />
          <div className="pipeline__run-bar">
            <Button
              variant="primary"
              ref={nextRef}
              busy={viaNext !== null && running === viaNext}
              disabled={!canRun || next === null}
              aria-describedby={reason ? reasonId : allPassed ? allPassedId : undefined}
              onClick={() => next && start({ kind: "step", id: next, viaNext: true })}
            >
              {next === null
                ? "Run next step"
                : `Run next: ${stepCopy[next].title}${stepCopy[next].confirm ? "…" : ""}`}
            </Button>
            <Button
              busy={runningAll}
              disabled={!canRun}
              aria-describedby={reason ? reasonId : undefined}
              onClick={() => start({ kind: "all" })}
            >
              {runningAll ? "Running all…" : "Run all…"}
            </Button>
            {reason ? (
              <p id={reasonId} className="pipeline__reason">
                {reason}
              </p>
            ) : (
              allPassed && (
                <p id={allPassedId} className="pipeline__reason">
                  All steps passed.
                </p>
              )
            )}
          </div>
          <ol className="pipeline__steps" aria-label="Steps">
            {session.steps.map((view, i) => (
              <StepItem
                key={view.id}
                ref={(el) => {
                  if (el) stepRefs.current.set(view.id, el);
                  else stepRefs.current.delete(view.id);
                }}
                number={i + 1}
                view={view}
                running={running === view.id}
                canRun={canRun}
                waitingFor={i > 0 ? stepCopy[session.steps[i - 1].id].title : null}
                onRun={() => start({ kind: "step", id: view.id })}
              />
            ))}
          </ol>
        </>
      )}
      <ConfirmDialog
        open={pending !== null}
        title={pending?.kind === "step" ? `Run ${stepCopy[pending.id].title}?` : "Run all steps?"}
        body={
          pending?.kind === "step" ? (
            <>
              {stepCopy[pending.id].confirm} Folder: <span className="mono">{confirmedDir}</span>
            </>
          ) : (
            <>
              Every step runs in order over <span className="mono">{confirmedDir}</span>, stopping at the
              first that does not pass.{" "}
              {session?.steps
                .map((v) => stepCopy[v.id].confirm)
                .filter(Boolean)
                .join(" ")}
            </>
          )
        }
        confirmLabel="Run"
        onCancel={() => setPending(null)}
        onConfirm={onConfirm}
      />
    </section>
  );
}

interface DirectoryFieldProps {
  slot: DirectorySlot | null;
  disabled: boolean;
  onBrowse: () => void;
  /** A typed path, cleaned; `null` clears the folder. Called on Enter or leaving the field, when changed. */
  onCommit: (path: string | null) => void;
}

function DirectoryField({ slot, disabled, onBrowse, onCommit }: DirectoryFieldProps) {
  const inputId = useId();
  const noteId = useId();
  const current = slot?.path ?? "";
  const [draft, setDraft] = useState(current);
  // Follow the session when it changes (browse): adjust state while rendering.
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
    <div className="pipeline__folder">
      <label htmlFor={inputId} className="pipeline__folder-title">
        Folder
      </label>
      <div className="pipeline__folder-row">
        <input
          id={inputId}
          className="pipeline__input mono"
          type="text"
          spellCheck={false}
          autoComplete="off"
          placeholder="C:\path\to\jobs"
          value={draft}
          disabled={disabled}
          aria-invalid={slot?.error ? true : undefined}
          aria-describedby={slot ? noteId : undefined}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={onKeyDown}
        />
        <Button onClick={onBrowse} disabled={disabled} aria-label="Browse for the folder">
          Browse…
        </Button>
      </div>
      {slot?.error ? (
        <p id={noteId} className="pipeline__field-error">
          {errorMessage(slot.error)}
        </p>
      ) : (
        slot && (
          <p id={noteId} className="pipeline__note">
            {slot.subdirectories} {plural(slot.subdirectories, "subfolder", "subfolders")}
          </p>
        )
      )}
    </div>
  );
}

interface StepItemProps {
  number: number;
  view: StepView;
  running: boolean;
  canRun: boolean;
  /** The previous step's title, shown while this step is locked. */
  waitingFor: string | null;
  onRun: () => void;
  ref: (el: HTMLElement | null) => void;
}

function StepItem({ number, view, running, canRun, waitingFor, onRun, ref }: StepItemProps) {
  const copy = stepCopy[view.id];
  const titleId = useId();
  const lockId = useId();
  const locked = !view.unlocked;
  const status = stepStatus(view, running);
  return (
    <li className="pipeline__step" ref={ref} tabIndex={-1} aria-labelledby={titleId}>
      <div className="pipeline__step-head">
        <span className="pipeline__step-number" aria-hidden="true">
          {number}
        </span>
        <div className="pipeline__step-text">
          <h2 id={titleId} className="pipeline__step-title">
            {copy.title}
          </h2>
          <p className="pipeline__step-description">{copy.description}</p>
          {locked && waitingFor && (
            <p id={lockId} className="pipeline__lock">
              Runs after {waitingFor} passes.
            </p>
          )}
        </div>
        <span className={`pipeline__status pipeline__status--${status.tone}`}>
          <Icon name={status.icon} />
          {status.label}
        </span>
        <Button
          busy={running}
          disabled={!canRun || locked}
          onClick={onRun}
          aria-label={`Run ${copy.title}`}
          aria-describedby={locked && waitingFor ? lockId : undefined}
        >
          {running ? "Running…" : copy.confirm ? "Run…" : "Run"}
        </Button>
      </div>
      {view.report && (
        <div className="pipeline__step-result">
          {view.report.summary && <p className="pipeline__summary">{view.report.summary}</p>}
          {view.report.issues.length > 0 && (
            <ul className="pipeline__issues" aria-label={`${copy.title} problems`}>
              {view.report.issues.map((issue, i) => (
                <li key={i}>
                  {issue.subdirectory !== null && <span className="mono">{issue.subdirectory}: </span>}
                  {issue.message}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      {view.error && (
        <div className="pipeline__step-result">
          <p
            className={view.error.kind === "StepNotImplemented" ? "pipeline__note" : "pipeline__field-error"}
          >
            {errorMessage(view.error)}
          </p>
          {view.error.kind === "StepFailed" && (
            <details className="banner__details">
              <summary>Details</summary>
              <p className="mono">{view.error.message}</p>
            </details>
          )}
        </div>
      )}
    </li>
  );
}
