import { useState } from "react";
import type { ToolEntry } from "../tools/registry";
import { ToolNav } from "./ToolNav";

interface AppShellProps {
  tools: ToolEntry[];
}

export function AppShell({ tools }: AppShellProps) {
  const [selectedId, setSelectedId] = useState<string | null>(tools[0]?.id ?? null);
  const active = tools.find((t) => t.id === selectedId) ?? tools[0];
  const View = active?.view;

  return (
    <div className="app-shell">
      <ToolNav tools={tools} activeId={active?.id ?? null} onSelect={setSelectedId} />
      <main className="app-shell__main">
        {View ? <View /> : <p className="app-shell__empty">No tools available.</p>}
      </main>
    </div>
  );
}
