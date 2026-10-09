import { useEffect, useId, useRef, type KeyboardEvent } from "react";
import { Button } from "../../app/Button";
import { useDevMode } from "../../app/devMode";

/** `devOnly` rows are listed only in developer mode. */
export const SHORTCUTS: { keys: string[][]; action: string; when: string; devOnly?: boolean }[] = [
  { keys: [["Ctrl", "O"]], action: "Open manifest…", when: "Always" },
  { keys: [["F5"], ["Ctrl", "R"]], action: "Reload manifest", when: "A manifest is open" },
  { keys: [["Ctrl", "E"]], action: "Edit in editor", when: "A manifest is open", devOnly: true },
  { keys: [["Ctrl", "Enter"]], action: "Create archive", when: "The archive can be built" },
  { keys: [["F8"]], action: "Show errors", when: "The manifest has errors" },
  { keys: [["Up"], ["Down"]], action: "Move between files", when: "Focus is in the table" },
  { keys: [["Enter"]], action: "Browse… for the focused file", when: "A file row is focused" },
  { keys: [["Delete"]], action: "Clear the focused file", when: "The row has a file assigned" },
  { keys: [["Esc"]], action: "Dismiss the drop or build result", when: "A result is showing" },
];

function isOpen(el: HTMLElement): boolean {
  try {
    if (el.matches(":popover-open")) return true;
  } catch {
    // Older engines and jsdom may not know the pseudo-class.
  }
  // The toggle event records the state as well; jsdom relies on it.
  return el.dataset.open === "true";
}

/** A quiet button that opens a non-modal popover listing every shortcut. */
export function ShortcutsHelp({ onOpenChange }: { onOpenChange?: (open: boolean) => void } = {}) {
  const id = useId();
  const devMode = useDevMode();
  const report = useRef(onOpenChange);
  useEffect(() => {
    report.current = onOpenChange;
  });
  // The header and no-manifest instances swap; an unmounted popover is closed.
  useEffect(() => () => report.current?.(false), []);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const popRef = useRef<HTMLDivElement>(null);

  const onKeyDown = (e: KeyboardEvent) => {
    const pop = popRef.current;
    if (e.key !== "Escape" || !pop || !isOpen(pop)) return;
    // Handled here: the view's Escape shortcut must not also dismiss a result.
    e.stopPropagation();
    pop.hidePopover?.();
    buttonRef.current?.focus();
  };

  return (
    // The span only catches Escape bubbling up from the button and popover; it is not a control.
    // eslint-disable-next-line jsx-a11y/no-static-element-interactions
    <span className="shortcuts" onKeyDown={onKeyDown}>
      <Button
        ref={buttonRef}
        variant="quiet"
        icon="keyboard"
        className="shortcuts__button"
        popoverTarget={id}
        aria-haspopup="dialog"
      >
        Keyboard shortcuts
      </Button>
      <div
        id={id}
        ref={popRef}
        popover="auto"
        className="shortcuts__popover"
        role="dialog"
        aria-label="Keyboard shortcuts"
        onToggle={(e) => {
          const el = e.currentTarget;
          const open = (e.nativeEvent as ToggleEvent).newState === "open";
          el.dataset.open = String(open);
          report.current?.(open);
          if (!isOpen(el) && el.contains(document.activeElement)) buttonRef.current?.focus();
        }}
      >
        <h2 className="shortcuts__title">Keyboard shortcuts</h2>
        <table className="shortcuts__table">
          <caption className="visually-hidden">Keyboard shortcuts</caption>
          <thead className="visually-hidden">
            <tr>
              <th scope="col">Keys</th>
              <th scope="col">Action</th>
              <th scope="col">Active when</th>
            </tr>
          </thead>
          <tbody>
            {SHORTCUTS.filter((s) => devMode || !s.devOnly).map((s) => (
              <tr key={s.action}>
                <td className="shortcuts__keys">
                  {s.keys.map((combo, i) => (
                    <span key={i}>
                      {i > 0 && <span className="shortcuts__or"> or </span>}
                      {combo.map((k, j) => (
                        <span key={k}>
                          {j > 0 && <span aria-hidden="true">+</span>}
                          <kbd>{k}</kbd>
                        </span>
                      ))}
                    </span>
                  ))}
                </td>
                <td>{s.action}</td>
                <td className="shortcuts__when">{s.when}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </span>
  );
}
