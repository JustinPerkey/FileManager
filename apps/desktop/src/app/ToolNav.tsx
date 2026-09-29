import { useRef, type KeyboardEvent } from "react";
import type { ToolEntry } from "../tools/registry";

interface ToolNavProps {
  tools: ToolEntry[];
  activeId: string | null;
  onSelect: (id: string) => void;
}

export function ToolNav({ tools, activeId, onSelect }: ToolNavProps) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  // Roving tabindex: the active item (or the first, if none) is the tab stop.
  const tabStopId = tools.some((t) => t.id === activeId) ? activeId : tools[0]?.id;

  function onKeyDown(e: KeyboardEvent<HTMLButtonElement>, index: number) {
    const last = tools.length - 1;
    let next: number;
    if (e.key === "ArrowDown") next = index === last ? 0 : index + 1;
    else if (e.key === "ArrowUp") next = index === 0 ? last : index - 1;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = last;
    else return;
    e.preventDefault();
    refs.current[next]?.focus();
  }

  return (
    <nav className="tool-nav" aria-label="Tools">
      <ul className="tool-nav__list">
        {tools.map((tool, i) => (
          <li key={tool.id}>
            <button
              type="button"
              className="tool-nav__item"
              ref={(el) => {
                refs.current[i] = el;
              }}
              tabIndex={tool.id === tabStopId ? 0 : -1}
              aria-current={tool.id === activeId ? "page" : undefined}
              onClick={() => onSelect(tool.id)}
              onKeyDown={(e) => onKeyDown(e, i)}
            >
              {tool.label}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
