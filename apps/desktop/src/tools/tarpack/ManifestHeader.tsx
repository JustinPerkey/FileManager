import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Button } from "../../app/Button";
import type { TarpackSession } from "../../lib/generated/TarpackSession";
import { recentManifests } from "../../lib/tarpack";
import { ShortcutsHelp } from "./ShortcutsHelp";

interface ManifestHeaderProps {
  session: TarpackSession;
  onOpen: () => void;
  onOpenRecent: (path: string) => void;
  onReload: () => void;
  onEdit: () => void;
}

/** Splits a path so the tail stays visible when the head is truncated. */
function splitPath(path: string): [string, string] {
  const tail = 28;
  const chars = Array.from(path);
  if (chars.length <= tail) return ["", path];
  return [chars.slice(0, chars.length - tail).join(""), chars.slice(-tail).join("")];
}

export function ManifestHeader({ session, onOpen, onOpenRecent, onReload, onEdit }: ManifestHeaderProps) {
  const manifest = session.manifest;
  const [recent, setRecent] = useState<string[]>([]);
  const [menuOpen, setMenuOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const focusIndex = useRef(0);

  // The list is re-read whenever the loaded manifest changes (open adds to it).
  const path = manifest?.path;
  useEffect(() => {
    let live = true;
    recentManifests().then(
      (r) => live && setRecent(r),
      () => live && setRecent([]),
    );
    return () => {
      live = false;
    };
  }, [path]);

  useEffect(() => {
    if (!menuOpen) return;
    const items = menuRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]');
    items?.[focusIndex.current]?.focus();
    const onDown = (e: MouseEvent) => {
      if (!menuRef.current?.contains(e.target as Node) && !triggerRef.current?.contains(e.target as Node))
        setMenuOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [menuOpen]);

  if (!manifest) return null;
  const [head, tail] = splitPath(manifest.path);

  const openMenu = (index: number) => {
    focusIndex.current = index;
    setMenuOpen(true);
  };
  const closeMenu = (refocus: boolean) => {
    setMenuOpen(false);
    if (refocus) triggerRef.current?.focus();
  };

  const onTriggerKey = (e: KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      openMenu(0);
    }
  };

  const onMenuKey = (e: KeyboardEvent) => {
    const items = Array.from(
      menuRef.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [],
    );
    const at = items.indexOf(document.activeElement as HTMLElement);
    let next: number | null = null;
    if (e.key === "ArrowDown") next = (at + 1) % items.length;
    else if (e.key === "ArrowUp") next = (at - 1 + items.length) % items.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = items.length - 1;
    else if (e.key === "Escape") {
      e.preventDefault();
      closeMenu(true);
      return;
    } else if (e.key === "Tab") {
      // Close and put focus on the trigger; the browser then moves on from there.
      closeMenu(true);
      return;
    }
    if (next !== null) {
      e.preventDefault();
      items[next]?.focus();
    }
  };

  return (
    <header className="manifest-header">
      <div className="manifest-header__title">
        <h1>{manifest.name}</h1>
        <p className="manifest-header__path" title={manifest.path}>
          <span className="manifest-header__head">{head}</span>
          <span className="manifest-header__tail">{tail}</span>
        </p>
      </div>
      <div className="manifest-header__actions">
        <Button title="Shortcut: Ctrl+O" aria-keyshortcuts="Control+O" onClick={onOpen}>
          Open…
        </Button>
        {recent.length > 0 && (
          <div className="menu">
            <Button
              ref={triggerRef}
              icon="chevron-down"
              iconAfter
              aria-haspopup="menu"
              aria-expanded={menuOpen}
              onClick={() => (menuOpen ? closeMenu(false) : openMenu(0))}
              onKeyDown={onTriggerKey}
            >
              Recent
            </Button>
            {menuOpen && (
              <div className="menu__list" role="menu" tabIndex={-1} aria-label="Recent manifests" ref={menuRef} onKeyDown={onMenuKey}>
                {recent.map((p) => (
                  <Button
                    key={p}
                    variant="quiet"
                    role="menuitem"
                    className="menu__item mono" tabIndex={-1}
                    title={p}
                    onClick={() => {
                      closeMenu(true);
                      onOpenRecent(p);
                    }}
                  >
                    {p}
                  </Button>
                ))}
              </div>
            )}
          </div>
        )}
        <Button title="Shortcut: F5 or Ctrl+R" aria-keyshortcuts="F5 Control+R" onClick={onReload}>
          Reload
        </Button>
        <Button title="Shortcut: Ctrl+E" aria-keyshortcuts="Control+E" onClick={onEdit}>
          Edit in editor
        </Button>
        <ShortcutsHelp />
      </div>
    </header>
  );
}
