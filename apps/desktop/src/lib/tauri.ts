import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";

/** Thin typed wrapper over `invoke`. Tool APIs (`lib/<tool>.ts`) build on it. */
export function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(command, args);
}

/** Thin typed wrapper over `listen`. */
export function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}

/**
 * A file-dialog filter. `extensions` are suffixes without the leading dot, and
 * Windows matches only the last one (for example `zst` for `.tar.zst`). The
 * backend's output normalisation is what guarantees the full extension.
 */
export interface DialogFilter {
  name: string;
  extensions: string[];
}

/** The native Open dialog for one file. Resolves to `null` when cancelled. */
export function openFileDialog(
  options: { filters?: DialogFilter[]; defaultPath?: string } = {},
): Promise<string | null> {
  return open({
    multiple: false,
    directory: false,
    filters: options.filters,
    defaultPath: options.defaultPath,
  });
}

/** The native Save dialog. Resolves to `null` when cancelled. */
export function saveFileDialog(options: {
  defaultPath: string;
  filters?: DialogFilter[];
}): Promise<string | null> {
  return save({ defaultPath: options.defaultPath, filters: options.filters });
}

/** A drag-and-drop event over the window. `paths` is empty except on `enter` and `drop`. */
export interface DragDrop {
  type: "enter" | "over" | "leave" | "drop";
  paths: string[];
}

/** Subscribes to files dragged over the window. */
export function onDragDrop(handler: (event: DragDrop) => void): Promise<UnlistenFn> {
  return getCurrentWebview().onDragDropEvent((e) => {
    const p = e.payload;
    handler({ type: p.type, paths: "paths" in p ? p.paths : [] });
  });
}
