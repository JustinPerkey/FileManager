import type { BuildSummary } from "../../lib/generated/BuildSummary";
import type { Diagnostic } from "../../lib/generated/Diagnostic";
import { fileName } from "./pathParts";

const plural = (n: number, one: string, many: string) => (n === 1 ? one : many);

/** The result panel's heading, also the first line of the copied report. */
export function resultHeading(s: BuildSummary): string {
  const name = fileName(s.path);
  if (s.errorCount === 0) return `Created ${name}`;
  if (s.leftOut.length > 0)
    return `Created ${name} with ${s.leftOut.length} ${plural(s.leftOut.length, "file", "files")} left out`;
  return `Created ${name} with ${s.manifestErrors.length} ${plural(s.manifestErrors.length, "manifest error", "manifest errors")}`;
}

/** The sentence announced once when a build resolves. */
export function resultAnnouncement(s: BuildSummary): string {
  const name = fileName(s.path);
  if (s.errorCount === 0) return `Created ${name}.`;
  if (s.leftOut.length > 0) {
    const n = s.leftOut.length;
    return `Created ${name}. ${n} ${n === 1 ? "file was" : "files were"} left out because of errors.`;
  }
  const n = s.manifestErrors.length;
  return `Created ${name} with ${n} ${plural(n, "manifest error", "manifest errors")}.`;
}

const diagLines = (list: Diagnostic[], indent: string) =>
  list.map((d) => `${indent}${d.line}:${d.col} ${d.message}`);

/** Plain text of the final report, for Copy report. Empty sections are omitted. */
export function reportText(s: BuildSummary): string {
  const out: string[] = [resultHeading(s)];
  if (s.leftOut.length > 0) {
    out.push("", "Left out of the archive:");
    for (const f of s.leftOut) {
      const name = f.id ?? `Entry #${f.index}`;
      const where = f.source !== null ? `source ${f.source}, line ${f.line}` : `line ${f.line}`;
      out.push(`${name} (${where})`, ...diagLines(f.errors, "  "));
    }
  }
  if (s.manifestErrors.length > 0) out.push("", "Manifest errors:", ...diagLines(s.manifestErrors, "  "));
  if (s.warnings.length > 0) out.push("", "Warnings:", ...diagLines(s.warnings, "  "));
  return out.join("\n");
}
