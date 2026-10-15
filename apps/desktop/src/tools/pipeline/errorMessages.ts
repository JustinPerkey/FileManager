import type { PipelineError } from "../../lib/generated/PipelineError";
import type { PipelineErrorKind } from "../../lib/generated/PipelineErrorKind";

/** One message per kind. A `Record` over the closed union: a missing or extra kind fails the typecheck. */
const messages: Record<PipelineErrorKind, string> = {
  NoDirectory: "Choose a folder first.",
  NotAbsolute: "Enter the full path, starting with the drive letter, for example C:\\.",
  NotADirectory: "No folder was found at this path.",
  DirectoryUnreadable:
    "The folder's contents could not be listed. Check that it still exists and you can open it.",
  DirectoryChanged: "The chosen folder changed before the step started. Nothing ran.",
  StepInProgress: "A step is still running. Wait for it to finish.",
  StepNotImplemented: "This step is not implemented yet.",
  StepFailed: "The step stopped before it finished. See Details.",
};

/**
 * An error the backend did not report as a `PipelineError` (an IPC or dialog
 * failure). Nothing is known about what happened, so the copy claims nothing.
 */
export interface UnexpectedError {
  kind: "Unexpected";
  message: string;
}
export type ViewError = PipelineError | UnexpectedError;

/** User-facing copy for an error. `error.message` is technical detail, shown separately. */
export function errorMessage(error: ViewError): string {
  if (error.kind === "Unexpected") return "Something went wrong. See Details.";
  return messages[error.kind];
}

/** A rejection may not be a `PipelineError` (for example, an IPC failure). */
export function toPipelineError(e: unknown): ViewError {
  if (typeof e === "object" && e !== null) {
    const o = e as Record<string, unknown>;
    if (typeof o.kind === "string" && Object.hasOwn(messages, o.kind) && typeof o.message === "string")
      return e as PipelineError;
  }
  return {
    kind: "Unexpected",
    message: e instanceof Error ? e.message : String(e),
  };
}
