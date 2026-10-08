import type { ScheduleError } from "../../lib/generated/ScheduleError";
import type { ScheduleErrorKind } from "../../lib/generated/ScheduleErrorKind";

/** One message per kind. A `Record` over the closed union: a missing or extra kind fails the typecheck. */
/** `text` ending in a full stop, unless it already ends in punctuation. */
const sentence = (text: string) => (/[.!?]$/.test(text.trim()) ? text.trim() : `${text.trim()}.`);

const messages: Record<ScheduleErrorKind, (e: ScheduleError) => string> = {
  NoText: () => "Choose a schedule text file first.",
  NoXml: () => "Choose the XML file to update first.",
  NotAFile: () => "No file was found at this path.",
  TextUnreadable: () =>
    "The schedule file could not be read. Check that it still exists and is saved as UTF-8 text.",
  XmlUnreadable: () =>
    "The XML file could not be read. Check that it still exists and is saved as UTF-8 text.",
  DropAmbiguous: () => "Drop one schedule file, one XML file, or one of each. Nothing was changed.",
  ParseFailed: (e) =>
    e.line !== undefined
      ? `Line ${e.line} of the schedule file: ${sentence(e.message)} Nothing was changed.`
      : `The schedule file has a problem: ${sentence(e.message)} Nothing was changed.`,
  ParseNotImplemented: () => "Reading schedule files is not implemented yet. Nothing was changed.",
  MergeFailed: () => "The schedule could not be added to the XML file. Nothing was changed.",
  MergeNotImplemented: () => "Updating the XML file is not implemented yet. Nothing was changed.",
  Io: () => "A backup or the updated XML file could not be written. The XML file was not changed.",
};

/**
 * An error the backend did not report as a `ScheduleError` (an IPC or dialog
 * failure). Nothing is known about what happened, so the copy claims nothing.
 */
export interface UnexpectedError {
  kind: "Unexpected";
  message: string;
}
export type ViewError = ScheduleError | UnexpectedError;

/** User-facing copy for an error. `error.message` is technical detail, shown separately. */
export function errorMessage(error: ViewError): string {
  if (error.kind === "Unexpected") return "Something went wrong. See Details.";
  return messages[error.kind](error);
}

/** A rejection may not be a `ScheduleError` (for example, an IPC failure). */
export function toScheduleError(e: unknown): ViewError {
  if (typeof e === "object" && e !== null) {
    const o = e as Record<string, unknown>;
    if (
      typeof o.kind === "string" &&
      Object.hasOwn(messages, o.kind) &&
      typeof o.message === "string" &&
      (o.line === undefined || typeof o.line === "number")
    )
      return e as ScheduleError;
  }
  return { kind: "Unexpected", message: e instanceof Error ? e.message : String(e) };
}

/** The last path component. Handles both separators; never assumes the path is valid. */
export function fileName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}
