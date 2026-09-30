import { flushSync } from "react-dom";
import { useCallback, useEffect, useImperativeHandle, useRef, useState, type Ref } from "react";
import { Banner } from "../../app/Banner";
import { Button } from "../../app/Button";
import { DropZone } from "../../app/DropZone";
import { openFileDialog, saveFileDialog } from "../../lib/tauri";
import type { DropOutcome } from "../../lib/generated/DropOutcome";
import type { TarpackError } from "../../lib/generated/TarpackError";
import type { TarpackSession } from "../../lib/generated/TarpackSession";
import {
  assign,
  assignDropped,
  clear,
  createManifestFromExample,
  onManifestChanged,
  openInEditor,
  openManifest,
  reloadManifest,
  session as fetchSession,
} from "../../lib/tarpack";
import { errorMessage, toTarpackError } from "./errorMessages";
import { DropResult, dropResultText } from "./DropResult";
import { ManifestErrors } from "./ManifestErrors";
import { EntryTable } from "./EntryTable";
import { ManifestHeader } from "./ManifestHeader";

/** Functions later tasks call on the view. */
export interface TarpackActions {
  showErrors: () => void;
  announce: (text: string) => void;
}

interface TarpackViewProps {
  actionsRef?: Ref<TarpackActions>;
  /** A build is running (wired in U5): drops are ignored. */
  building?: boolean;
}

const TOML = [{ name: "Manifest", extensions: ["toml"] }];
const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

type Origin = "load" | "reload" | "other";

function manifestSentence(s: TarpackSession, prevErrors: number, origin: Origin): string | null {
  const m = s.manifest;
  if (!m || origin === "other") return null;
  if (m.errorCount > 0) {
    let text = `${m.name} has ${m.errorCount} ${plural(m.errorCount, "error", "errors")}.`;
    if (m.entriesWithheld) text += " No files can be built until the manifest errors are fixed.";
    else if (m.failedEntries.length > 0) {
      const n = m.failedEntries.length;
      text += ` ${n} ${plural(n, "file will", "files will")} be left out of the archive.`;
    }
    return text;
  }
  if (origin === "reload" && prevErrors > 0) return `${m.name} reloaded. No errors.`;
  return null;
}

export function TarpackView({ actionsRef, building = false }: TarpackViewProps) {
  const [session, setSession] = useState<TarpackSession | null>(null);
  const [showSkeleton, setShowSkeleton] = useState(false);
  const [commandError, setCommandError] = useState<TarpackError | null>(null);
  const [failureCount, setFailureCount] = useState(0);
  const [restoreFailed, setRestoreFailed] = useState(false);
  const [changed, setChanged] = useState(false);
  const changedRef = useRef(false);
  const [stateWarningDismissed, setStateWarningDismissed] = useState<string | null>(null);
  const dismissedRef = useRef<string | null>(null);
  const [expanded, setExpanded] = useState(true);
  const [dropOutcome, setDropOutcome] = useState<DropOutcome | null>(null);
  const [announcement, setAnnouncement] = useState("");
  const reportRef = useRef<HTMLDivElement>(null);
  const sessionRef = useRef<TarpackSession | null>(null);
  const frame = useRef(0);

  const announce = useCallback((text: string) => {
    cancelAnimationFrame(frame.current);
    setAnnouncement("");
    frame.current = requestAnimationFrame(() => setAnnouncement(text));
  }, []);

  const apply = useCallback(
    (next: TarpackSession, origin: Origin) => {
      const prev = sessionRef.current;
      const prevErrors = prev?.manifest?.errorCount ?? 0;
      sessionRef.current = next;
      setSession(next);
      setCommandError(null);
      if (origin !== "other") {
        changedRef.current = false;
        setChanged(false);
        setDropOutcome(null);
        if ((next.manifest?.errorCount ?? 0) > 0) setExpanded(true);
      }
      // One announce() call per result: a second call would lose the first.
      const parts: string[] = [];
      const sw = next.stateWarning;
      if (sw && sw !== prev?.stateWarning && sw !== dismissedRef.current) parts.push(sw);
      const sentence = manifestSentence(next, prevErrors, origin);
      if (sentence) parts.push(sentence);
      if (parts.length > 0) announce(parts.join(" "));
    },
    [announce],
  );

  const fail = useCallback((e: unknown) => {
    setCommandError(toTarpackError(e));
    setFailureCount((n) => n + 1);
  }, []);

  // First session() call: restore.
  useEffect(() => {
    let live = true;
    const timer = setTimeout(() => live && setShowSkeleton(true), 150);
    fetchSession().then(
      (s) => live && apply(s, "load"),
      (e) => live && (setRestoreFailed(true), fail(e)),
    );
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [apply, fail]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let live = true;
    onManifestChanged(() => {
      if (!changedRef.current) announce("The manifest changed on disk.");
      changedRef.current = true;
      setChanged(true);
    }).then(
      (u) => (live ? (unlisten = u) : u()),
      () => undefined,
    );
    return () => {
      live = false;
      unlisten?.();
    };
  }, [announce]);

  useEffect(() => () => cancelAnimationFrame(frame.current), []);

  const showErrors = useCallback(() => {
    if ((sessionRef.current?.manifest?.errorCount ?? 0) === 0) return;
    flushSync(() => setExpanded(true));
    const el = reportRef.current;
    if (!el) return;
    el.focus({ preventScroll: true });
    const reduce =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    el.scrollIntoView?.({
      behavior: reduce ? "instant" : "smooth",
      block: "nearest",
    });
  }, []);

  useImperativeHandle(actionsRef, () => ({ showErrors, announce }), [showErrors, announce]);

  const run = useCallback(
    async (fn: () => Promise<TarpackSession>, origin: Origin) => {
      try {
        apply(await fn(), origin);
      } catch (e) {
        fail(e);
      }
    },
    [apply, fail],
  );

  const onOpen = useCallback(async () => {
    try {
      const path = await openFileDialog({ filters: TOML });
      if (path) await run(() => openManifest(path), "load");
    } catch (e) {
      fail(e);
    }
  }, [run, fail]);
  const onOpenRecent = useCallback((path: string) => run(() => openManifest(path), "load"), [run]);
  const onReload = useCallback(() => run(reloadManifest, "reload"), [run]);
  const onEdit = useCallback(async () => {
    try {
      await openInEditor();
    } catch (e) {
      fail(e);
    }
  }, [fail]);
  const onCreate = useCallback(async () => {
    try {
      const path = await saveFileDialog({
        defaultPath: "manifest.toml",
        filters: TOML,
      });
      if (path) await run(() => createManifestFromExample(path), "load");
    } catch (e) {
      fail(e);
    }
  }, [run, fail]);

  const onBrowse = useCallback(
    async (id: string) => {
      const assigned = sessionRef.current?.manifest?.entries.find((e) => e.id === id)?.assigned;
      try {
        const path = await openFileDialog({ defaultPath: folderOf(assigned) });
        if (path) await run(() => assign(id, path), "other");
      } catch (e) {
        fail(e);
      }
    },
    [run, fail],
  );
  const onClear = useCallback((id: string) => run(() => clear(id), "other"), [run]);

  const onDrop = useCallback(
    async (paths: string[]) => {
      try {
        const { session: next, outcome } = await assignDropped(paths);
        apply(next, "other");
        setDropOutcome(outcome);
        announce(dropResultText(outcome, next.manifest?.failedEntries.length ? true : false));
      } catch (e) {
        fail(e);
      }
    },
    [apply, announce, fail],
  );

  const loading = session === null && !restoreFailed;
  const manifest = session?.manifest ?? null;
  const dropState = dropAvailability(manifest, building);
  const hasFailedEntries = (manifest?.failedEntries.length ?? 0) > 0;
  const stateWarning =
    session?.stateWarning && session.stateWarning !== stateWarningDismissed ? session.stateWarning : null;

  return (
    <section className="tool-view tarpack" aria-busy={loading ? true : undefined}>
      <Announcer text={announcement} />
      <DropZone
        enabled={dropState.enabled}
        disabledReason={dropState.reason}
        label="Drop files or folders to match them to the manifest"
        onDrop={onDrop}
      />
      {loading ? (
        <>
          <span className="visually-hidden">Loading manifest</span>
          {showSkeleton && (
            <div className="skeleton" aria-hidden="true">
              <div className="skeleton__bar skeleton__bar--wide" />
              <div className="skeleton__bar" />
            </div>
          )}
        </>
      ) : manifest && session ? (
        <ManifestHeader
          session={session}
          onOpen={onOpen}
          onOpenRecent={onOpenRecent}
          onReload={onReload}
          onEdit={onEdit}
        />
      ) : (
        <h1>Tar Packager</h1>
      )}
      {!loading && (
        <div className="banners">
          {commandError && (
            <CommandError
              key={failureCount}
              error={commandError}
              session={session}
              onDismiss={() => setCommandError(null)}
            />
          )}
          {changed && manifest && (
            <Banner
              tone="warn"
              message="The manifest changed on disk."
              action={{ label: "Reload", onAction: onReload }}
            />
          )}
          {stateWarning && (
            <Banner
              tone="info"
              message={stateWarning}
              onDismiss={() => {
                dismissedRef.current = stateWarning;
                setStateWarningDismissed(stateWarning);
              }}
            />
          )}
        </div>
      )}
      {!loading && !manifest && (
        <div className="empty">
          <p>Open a manifest to list the files this package needs.</p>
          <div className="empty__actions">
            <Button variant="primary" onClick={onOpen}>
              Open manifest…
            </Button>
            <Button onClick={onCreate}>Create from example…</Button>
          </div>
          <p className="empty__hint">
            A manifest is a TOML file that lists each file, its Linux path, and its permissions.
          </p>
        </div>
      )}
      {manifest && (
        <ManifestErrors
          manifest={manifest}
          expanded={expanded}
          onExpandedChange={setExpanded}
          onEdit={onEdit}
          reportRef={reportRef}
        />
      )}
      {manifest && dropOutcome && (
        <DropResult
          outcome={dropOutcome}
          hasFailedEntries={hasFailedEntries}
          onDismiss={() => setDropOutcome(null)}
        />
      )}
      {manifest && (
        <EntryTable
          entries={manifest.entries}
          failedCount={manifest.failedEntries.length}
          entriesWithheld={manifest.entriesWithheld}
          onBrowse={onBrowse}
          onClear={onClear}
        />
      )}
      {/* Slot for a later task: build result and bar (U5). */}
    </section>
  );
}

/** The folder part of a display path, or undefined when there is none or the path is lossy. */
function folderOf(path: string | null | undefined): string | undefined {
  if (!path || path.includes("\uFFFD")) return undefined;
  const i = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return i > 0 ? path.slice(0, i) : undefined;
}

function dropAvailability(
  manifest: TarpackSession["manifest"],
  building: boolean,
): { enabled: boolean; reason: string } {
  if (!manifest) return { enabled: false, reason: "Open a manifest first" };
  if (manifest.entries.length === 0) {
    return {
      enabled: false,
      reason:
        manifest.entriesWithheld || manifest.failedEntries.length > 0
          ? "Fix the manifest errors first. No files can be matched yet."
          : "This manifest lists no files",
    };
  }
  if (building) return { enabled: false, reason: "A build is running" };
  return { enabled: true, reason: "" };
}

function Announcer({ text }: { text: string }) {
  return (
    <div className="visually-hidden" role="status" aria-live="polite">
      {text}
    </div>
  );
}

function CommandError({
  error,
  session,
  onDismiss,
}: {
  error: TarpackError;
  session: TarpackSession | null;
  onDismiss: () => void;
}) {
  return (
    <Banner
      tone="error"
      message={errorMessage(error, session?.manifest?.entries ?? [])}
      onDismiss={onDismiss}
    >
      <details className="banner__details">
        <summary>Details</summary>
        <p className="mono">{error.message}</p>
      </details>
    </Banner>
  );
}
