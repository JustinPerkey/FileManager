import type { Diagnostic } from "../../lib/generated/Diagnostic";

interface DiagnosticListProps {
  diagnostics: Diagnostic[];
  label: string;
}

export function DiagnosticList({ diagnostics, label }: DiagnosticListProps) {
  if (diagnostics.length === 0) return null;
  return (
    <ul className="diag-list" aria-label={label}>
      {diagnostics.map((d, i) => (
        <li key={i} className="diag-list__item">
          <span className="visually-hidden">
            Line {d.line}, column {d.col}:{" "}
          </span>
          <span className="diag-list__pos num" aria-hidden="true">
            {d.line}:{d.col}
          </span>
          <span className="diag-list__msg">{d.message}</span>
        </li>
      ))}
    </ul>
  );
}
