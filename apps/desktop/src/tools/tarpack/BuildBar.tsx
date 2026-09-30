import { useId } from "react";
import { Button } from "../../app/Button";
import { Icon } from "../../app/icons";
import type { ArchiveFormat } from "../../lib/generated/ArchiveFormat";
import type { Progress } from "../../lib/generated/Progress";
import type { TarpackSession } from "../../lib/generated/TarpackSession";
import { saveFileDialog } from "../../lib/tauri";
import { BuildProgress } from "./BuildProgress";
import { FORMAT_NAME, FormatPicker } from "./FormatPicker";
import { MiddlePath } from "./MiddlePath";

interface BuildBarProps {
  session: TarpackSession;
  building: boolean;
  progress: Progress | null;
  /** The `onBuildProgress` subscription failed: say so instead of showing a bar stuck at 0%. */
  progressUnavailable?: boolean;
  /** Called with a path chosen in the Save dialog; a cancelled dialog calls nothing. May reject. */
  onChooseOutput: (path: string) => void | Promise<void>;
  /** May reject; the rejection goes to `onError`. */
  onFormatChange: (format: ArchiveFormat) => void | Promise<void>;
  /** Every rejection from the Save dialog, `setOutput`, or `setFormat`; the view shows it in its banner. */
  onError: (e: unknown) => void;
  onBuild: () => void;
  onShowErrors: () => void;
  /** Set when the file name switched the format; read out in the bar's live region. */
  formatNotice?: string;
}

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

function reasonText(s: TarpackSession): string | null {
  switch (s.buildBlockedReason) {
    case null:
      return null;
    case "noManifest":
      return "Open a manifest first";
    case "noEntries": {
      const m = s.manifest;
      if (m?.entriesWithheld) return "No files can be built until the manifest errors are fixed";
      if (m && m.failedEntries.length > 0) return "Every file in the manifest has errors";
      return "This manifest lists no files";
    }
    case "entriesNotReady": {
      const n = s.totalCount - s.readyCount;
      return `${n} ${plural(n, "file still needs", "files still need")} a location`;
    }
    case "noOutput":
      return "Choose where to save the archive";
  }
}

function leftOutNote(s: TarpackSession): string | null {
  const m = s.manifest;
  if (!m || m.errorCount === 0 || m.entries.length === 0) return null;
  const n = m.failedEntries.length;
  if (n > 0)
    return `${n} ${plural(n, "file", "files")} will be left out; errors will be listed after the build`;
  const e = m.errors.length;
  return `Builds with ${e} manifest ${plural(e, "error", "errors")}`;
}

export function BuildBar({
  session,
  building,
  progress,
  progressUnavailable = false,
  onChooseOutput,
  onFormatChange,
  onBuild,
  onShowErrors,
  onError,
  formatNotice,
}: BuildBarProps) {
  const reasonId = useId();
  const noteId = useId();
  const manifest = session.manifest;
  const reason = reasonText(session);
  const note = leftOutNote(session);
  const errorCount = manifest?.errorCount ?? 0;
  // While building, the reason and note are not rendered, so no id may point at them.
  const describedBy = building
    ? ""
    : [reason ? reasonId : null, note ? noteId : null].filter(Boolean).join(" ");

  async function choose() {
    try {
      await chooseUnsafe();
    } catch (e) {
      onError(e);
    }
  }

  async function chooseUnsafe() {
    const current = session.formats.find((f) => f.format === session.format);
    const defaultPath = session.outputPath ?? session.suggestedOutputName;
    if (!current || defaultPath === null) return;
    const path = await saveFileDialog({
      defaultPath,
      filters: [
        {
          name: `${FORMAT_NAME[current.format]} archive (${current.extension})`,
          extensions: [current.filterExtension],
        },
      ],
    });
    if (path) await onChooseOutput(path);
  }

  return (
    <div className="build-bar">
      <div className="build-bar__settings">
        <div className="build-bar__output">
          <span className="build-bar__output-label">Output:</span>
          <span className="build-bar__path">
            {session.outputPath ? (
              <MiddlePath path={session.outputPath} />
            ) : (
              <span className="build-bar__none">not chosen</span>
            )}
          </span>
          <Button icon="folder" disabled={!manifest || building} onClick={() => void choose()}>
            Choose…
          </Button>
        </div>
        <FormatPicker
          formats={session.formats}
          value={session.format}
          disabled={!manifest || building}
          onChange={(f) => {
            void (async () => {
              try {
                await onFormatChange(f);
              } catch (e) {
                onError(e);
              }
            })();
          }}
        />
      </div>
      <div className="build-bar__action">
        {building && progressUnavailable ? (
          <p className="build-bar__reason">Creating the archive… (progress is unavailable)</p>
        ) : building ? (
          <BuildProgress progress={progress} entries={manifest?.entries ?? []} />
        ) : (
          <div className="build-bar__status">
            {reason && (
              <p id={reasonId} className="build-bar__reason">
                {reason}
              </p>
            )}
            {note && (
              <p id={noteId} className="build-bar__note">
                <span className="build-bar__note-icon">
                  <Icon name="alert-triangle" />
                </span>
                {note}
              </p>
            )}
          </div>
        )}
        {errorCount > 0 && (
          <Button aria-controls="manifest-error-report" disabled={building} onClick={onShowErrors}>
            Show errors
          </Button>
        )}
        <Button
          variant="primary"
          className="build-bar__create"
          disabled={!session.canBuild || building}
          busy={building}
          aria-describedby={describedBy || undefined}
          onClick={onBuild}
        >
          Create archive
        </Button>
      </div>
      <div className="visually-hidden" role="status" aria-live="polite">
        {formatNotice ?? ""}
      </div>
    </div>
  );
}
