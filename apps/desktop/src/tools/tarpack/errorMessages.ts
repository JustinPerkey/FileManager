import type { SessionEntry } from "../../lib/generated/SessionEntry";
import type { TarpackError } from "../../lib/generated/TarpackError";
import type { TarpackErrorKind } from "../../lib/generated/TarpackErrorKind";

const file = (source: string | null) => source ?? "A file";

/** One message per kind. A `Record` over the closed union: a missing or extra kind fails the typecheck. */
const messages: Record<TarpackErrorKind, (source: string | null) => string> = {
  NoManifest: () => "Open a manifest first.",
  ManifestUnreadable: () =>
    "The manifest could not be read. Check that the file still exists and that you can open it.",
  NoEntries: () => "There are no files that can be built. Fix the manifest errors first.",
  ManifestChangedOnDisk: () => "The manifest changed on disk. Reload it, then build again.",
  UnknownEntry: () => "That file is no longer in the manifest. Reload and try again.",
  NotAFile: (s) => `${file(s)}: the chosen path is not a file.`,
  NoOutput: () => "Choose where to save the archive.",
  EntriesNotReady: (s) => `${file(s)} still needs a location.`,
  OutputExists: () => "A file with this name already exists.",
  PathExists: () =>
    "A file already exists there. Choose a new name; the example never replaces a file.",
  SourceMissing: (s) => `${file(s)} is no longer at its assigned location.`,
  SourceUnreadable: (s) => `${file(s)} could not be read.`,
  SourceChanged: (s) =>
    `${file(s)} changed while the archive was being written. Nothing was saved. Try again.`,
  VerifyFailed: () =>
    "The archive failed its check after writing, so it was not saved. Any existing file was left unchanged. Try again.",
  BuildInProgress: () => "A build is already running.",
  OpenerFailed: () => "Windows could not open it.",
  Io: () => "A file could not be read or written.",
};

/** User-facing copy for a command error. `error.message` is technical detail, shown separately. */
export function errorMessage(error: TarpackError, entries: SessionEntry[]): string {
  const source = entries.find((e) => e.id === error.entryId)?.source ?? null;
  return messages[error.kind](source);
}

/**
 * Temporary guard: `lib/tarpack.ts` passes rejections through unchanged, so
 * a rejection may not be a `TarpackError`. Remove when `lib/` guarantees it.
 */
export function toTarpackError(e: unknown): TarpackError {
  if (typeof e === "object" && e !== null) {
    const o = e as Record<string, unknown>;
    if (
      typeof o.kind === "string" &&
      Object.hasOwn(messages, o.kind) &&
      typeof o.message === "string" &&
      (o.entryId === undefined || typeof o.entryId === "string")
    )
      return e as TarpackError;
  }
  return { kind: "Io", message: e instanceof Error ? e.message : String(e) };
}
