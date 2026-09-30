import { useEffect, useRef } from "react";

export interface TarpackShortcutOptions {
  /** False while the confirmation dialog is open or a build is running. */
  active: boolean;
  hasManifest: boolean;
  canBuild: boolean;
  errorCount: number;
  dropResultVisible: boolean;
  buildResultVisible: boolean;
  /** The shortcuts popover is open: Escape belongs to it. */
  shortcutsOpen: boolean;
  onOpen: () => void;
  onReload: () => void;
  onEdit: () => void;
  onBuild: () => void;
  onShowErrors: () => void;
  onDismissDropResult: () => void;
  onDismissBuildResult: () => void;
}

/**
 * The tool's global shortcuts, on `window`. Row keys (Up, Down, Enter, Delete)
 * belong to `EntryTable`. The handler reads the latest options from a ref, so
 * the listener is bound once.
 */
export function useTarpackShortcuts(options: TarpackShortcutOptions) {
  const ref = useRef(options);
  useEffect(() => {
    ref.current = options;
  });

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const o = ref.current;
      const ctrl = e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey;
      const bare = !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey;
      const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;

      // The webview must never reload, whatever the state or the modifiers.
      if (key === "F5" || (e.ctrlKey && key === "r")) {
        e.preventDefault();
        const plain = (bare && key === "F5") || (ctrl && key === "r");
        if (plain && o.active && o.hasManifest && !e.repeat) o.onReload();
        return;
      }
      if (!o.active || e.defaultPrevented) return;

      let run: (() => void) | null = null;
      if (ctrl && key === "o") run = o.onOpen;
      else if (ctrl && key === "e") run = o.hasManifest ? o.onEdit : null;
      else if (ctrl && key === "Enter") run = o.canBuild ? o.onBuild : null;
      else if (bare && key === "F8") run = o.hasManifest && o.errorCount > 0 ? o.onShowErrors : null;
      else if (bare && key === "Escape") {
        // The popover's own light dismiss handles this Escape.
        if (o.shortcutsOpen) return;
        if (o.dropResultVisible) run = o.onDismissDropResult;
        else if (o.buildResultVisible) run = o.onDismissBuildResult;
      }
      if (!run) {
        // Claim the chord even when inactive, so the webview does not act on Ctrl+O or Ctrl+E.
        if (ctrl && (key === "o" || key === "e")) e.preventDefault();
        return;
      }
      e.preventDefault();
      if (!e.repeat) run();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
