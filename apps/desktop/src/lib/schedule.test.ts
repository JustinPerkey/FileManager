import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

import * as schedule from "./schedule";

beforeEach(() => {
  vi.clearAllMocks();
  invoke.mockResolvedValue(undefined);
});

describe("schedule client", () => {
  it.each([
    ["session", () => schedule.session(), "schedule_session", undefined],
    ["openText", () => schedule.openText("s.txt"), "schedule_open_text", { path: "s.txt" }],
    ["reloadText", () => schedule.reloadText(), "schedule_reload_text", undefined],
    ["openXml", () => schedule.openXml("s.xml"), "schedule_open_xml", { path: "s.xml" }],
    ["apply", () => schedule.apply(), "schedule_apply", undefined],
  ])("%s invokes the right command", async (_name, run, command, args) => {
    await run();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith(command, args);
  });
});
