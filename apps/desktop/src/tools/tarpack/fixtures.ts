import type { Diagnostic } from "../../lib/generated/Diagnostic";
import type { EntryFailure } from "../../lib/generated/EntryFailure";
import type { SessionEntry } from "../../lib/generated/SessionEntry";
import type { SessionManifest } from "../../lib/generated/SessionManifest";
import type { TarpackSession } from "../../lib/generated/TarpackSession";

export const diag = (message: string, line = 3, col = 1, entryId: string | null = null): Diagnostic => ({
  severity: "error",
  line,
  col,
  entryId,
  message,
});

export const entry = (id: string, source = `${id}.bin`): SessionEntry => ({
  assigned: null,
  status: "unassigned",
  id,
  source,
  targetPath: `/opt/${id}`,
  mode: "0755",
  modeText: "rwxr-xr-x",
  owner: "root:root",
  uid: 0,
  gid: 0,
  normalizeEol: false,
});

export const failure = (index: number, over: Partial<EntryFailure> = {}): EntryFailure => ({
  index,
  id: `bad${index}`,
  source: `bad${index}.bin`,
  line: index * 5,
  errors: [diag(`file \`bad${index}\`: missing field \`dir\``, index * 5 + 1, 3)],
  ...over,
});

export function manifest(over: Partial<SessionManifest> = {}): SessionManifest {
  const m: SessionManifest = {
    path: "C:\\pkgs\\gateway\\manifest.toml",
    name: "gateway",
    outputName: null,
    hash: "abc",
    entries: [entry("gateway")],
    entriesWithheld: false,
    errors: [],
    failedEntries: [],
    errorCount: 0,
    warnings: [],
    ...over,
  };
  if (over.errorCount === undefined)
    m.errorCount = m.errors.length + m.failedEntries.reduce((n, f) => n + f.errors.length, 0);
  return m;
}

export function session(m: SessionManifest | null, over: Partial<TarpackSession> = {}): TarpackSession {
  return {
    manifest: m,
    outputPath: null,
    format: "tar",
    formats: [],
    suggestedOutputName: null,
    readyCount: 0,
    totalCount: m?.entries.length ?? 0,
    canBuild: false,
    buildBlockedReason: null,
    stateWarning: null,
    ...over,
  } as TarpackSession;
}
