import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const listen = vi.fn();
const open = vi.fn();
const save = vi.fn();
const onDragDropEvent = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: (...a: unknown[]) => listen(...a) }));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: (...a: unknown[]) => onDragDropEvent(...a) }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...a: unknown[]) => open(...a),
  save: (...a: unknown[]) => save(...a),
}));

import { call, on, onDragDrop, openFileDialog, saveFileDialog } from "./tauri";

beforeEach(() => {
  vi.clearAllMocks();
});

describe("tauri wrappers", () => {
  it("call forwards the command and args", async () => {
    invoke.mockResolvedValue(7);
    await expect(call<number>("cmd", { a: 1 })).resolves.toBe(7);
    expect(invoke).toHaveBeenCalledWith("cmd", { a: 1 });
  });

  it("on hands the payload to the handler", async () => {
    const unlisten = vi.fn();
    listen.mockImplementation((_e: string, cb: (e: { payload: string }) => void) => {
      cb({ payload: "p" });
      return Promise.resolve(unlisten);
    });
    const handler = vi.fn();
    await expect(on<string>("evt", handler)).resolves.toBe(unlisten);
    expect(listen).toHaveBeenCalledWith("evt", expect.any(Function));
    expect(handler).toHaveBeenCalledWith("p");
  });

  it("openFileDialog opens a single file with filters", async () => {
    open.mockResolvedValue("C:\\a.toml");
    const filters = [{ name: "Manifest", extensions: ["toml"] }];
    await expect(openFileDialog({ filters, defaultPath: "C:\\" })).resolves.toBe("C:\\a.toml");
    expect(open).toHaveBeenCalledWith({
      multiple: false,
      directory: false,
      filters,
      defaultPath: "C:\\",
    });
  });

  it("saveFileDialog passes defaultPath and filters", async () => {
    save.mockResolvedValue(null);
    const filters = [{ name: "Zstd archive", extensions: ["zst"] }];
    await expect(saveFileDialog({ defaultPath: "x.tar.zst", filters })).resolves.toBeNull();
    expect(save).toHaveBeenCalledWith({ defaultPath: "x.tar.zst", filters });
  });

  it("onDragDrop yields the type and paths", async () => {
    const unlisten = vi.fn();
    onDragDropEvent.mockImplementation((cb: (e: { payload: unknown }) => void) => {
      cb({ payload: { type: "drop", paths: ["C:\\a"], position: { x: 0, y: 0 } } });
      cb({ payload: { type: "leave" } });
      return Promise.resolve(unlisten);
    });
    const handler = vi.fn();
    await expect(onDragDrop(handler)).resolves.toBe(unlisten);
    expect(handler).toHaveBeenNthCalledWith(1, { type: "drop", paths: ["C:\\a"] });
    expect(handler).toHaveBeenNthCalledWith(2, { type: "leave", paths: [] });
  });
});
