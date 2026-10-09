import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

export const DevModeContext = createContext(false);

/** True while developer mode is on: shows tools meant for the people who maintain packages. */
export const useDevMode = () => useContext(DevModeContext);

/** Ctrl+Alt+Shift+D. Deliberately absent from the shortcuts list. */
export function isDevModeChord(e: KeyboardEvent): boolean {
  return e.ctrlKey && e.altKey && e.shiftKey && !e.metaKey && e.code === "KeyD";
}

/**
 * Holds developer mode for the whole app. It starts off on every launch and is
 * toggled by a chord on `window`; the change is announced, since nothing else
 * says why controls appeared or went away.
 */
export function DevModeProvider({ initial = false, children }: { initial?: boolean; children: ReactNode }) {
  const [{ on, announcement }, setState] = useState({ on: initial, announcement: "" });

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (!isDevModeChord(e)) return;
      e.preventDefault();
      if (e.repeat) return;
      setState((s) => ({ on: !s.on, announcement: s.on ? "Developer mode off." : "Developer mode on." }));
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <DevModeContext.Provider value={on}>
      {children}
      <div className="visually-hidden" role="status" aria-live="polite" data-testid="dev-mode-announcer">
        {announcement}
      </div>
    </DevModeContext.Provider>
  );
}
