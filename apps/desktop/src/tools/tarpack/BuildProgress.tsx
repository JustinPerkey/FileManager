import { useState } from "react";
import type { Progress } from "../../lib/generated/Progress";
import type { SessionEntry } from "../../lib/generated/SessionEntry";

interface BuildProgressProps {
  /** `null` until the first event arrives: the writing phase at 0%. */
  progress: Progress | null;
  entries: SessionEntry[];
}

export function BuildProgress({ progress, entries }: BuildProgressProps) {
  const phase = progress?.phase ?? "writing";
  const entryId = progress?.entryId ?? null;
  // Keep the last named file, so directory and long-name records do not blank the label.
  const [lastId, setLastId] = useState<string | null>(null);
  if (entryId !== null && entryId !== lastId) setLastId(entryId);
  const shownId = entryId ?? lastId;
  const source = shownId === null ? null : (entries.find((e) => e.id === shownId)?.source ?? null);

  const done = progress !== null && progress.bytesDone >= progress.bytesTotal;
  const finishing = phase === "verifying" && done;
  const fraction =
    progress === null
      ? 0
      : progress.bytesTotal > 0
        ? Math.min(1, progress.bytesDone / progress.bytesTotal)
        : 1;
  const percent = Math.floor(fraction * 100);
  const word = phase === "writing" ? "Writing" : "Verifying";
  const valueText = finishing
    ? "Finishing, 100%"
    : `${word}${source && !done ? ` ${source}` : ""}, ${percent}%`;

  return (
    <div className="build-progress">
      <p className="build-progress__label">
        {finishing ? (
          "Finishing…"
        ) : (
          <>
            Step {phase === "writing" ? 1 : 2} of 2 · {word}
            {source && !done && (
              <>
                {" "}
                <span className="mono build-progress__file">{source}</span>
              </>
            )}
          </>
        )}
      </p>
      <div className="build-progress__row">
        <div
          className="build-progress__track"
          role="progressbar"
          aria-label="Building archive"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
          aria-valuetext={valueText}
        >
          <div className="build-progress__fill" style={{ transform: `scaleX(${fraction})` }} />
        </div>
        <span className="build-progress__percent num" aria-hidden="true">
          {percent}%
        </span>
      </div>
      <div className="visually-hidden" role="status" aria-live="polite">
        {phase === "verifying" ? "Verifying the archive" : ""}
      </div>
    </div>
  );
}
