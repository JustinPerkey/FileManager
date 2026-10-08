import { useEffect, useRef, useState } from "react";
import { onDragDrop } from "../lib/tauri";
import { Icon } from "./icons";

interface DropZoneProps {
  enabled: boolean;
  /** Shown in place of `label` while disabled. */
  disabledReason: string;
  label: string;
  onDrop: (paths: string[]) => void;
}

/**
 * Full-window drop target. It renders nothing until a drag is over the window,
 * then a non-interactive overlay (no focus, no pointer events) with its label.
 * Disabled, the overlay still shows, with the reason, and drops are ignored.
 */
export function DropZone({ enabled, disabledReason, label, onDrop }: DropZoneProps) {
  const [over, setOver] = useState(false);
  const enabledRef = useRef(enabled);
  const onDropRef = useRef(onDrop);
  useEffect(() => {
    enabledRef.current = enabled;
    onDropRef.current = onDrop;
  });

  useEffect(() => {
    let live = true;
    let unlisten: (() => void) | undefined;
    // Outside the Tauri webview there is no drag-drop source; subscribing throws.
    Promise.resolve()
      .then(() =>
        onDragDrop((event) => {
          if (event.type === "enter" || event.type === "over") setOver(true);
          else {
            setOver(false);
            if (event.type === "drop" && enabledRef.current && event.paths.length > 0) {
              onDropRef.current(event.paths);
            }
          }
        }),
      )
      .then(
        (u) => (live ? (unlisten = u) : u()),
        () => undefined,
      );
    return () => {
      live = false;
      unlisten?.();
    };
  }, []);

  if (!over) return null;
  return (
    <div className="drop-zone" data-enabled={enabled}>
      <p className="drop-zone__plate">
        <Icon name={enabled ? "folder" : "info"} />
        <span>{enabled ? label : disabledReason}</span>
      </p>
    </div>
  );
}
