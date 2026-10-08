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
    ["setText", () => schedule.setText("s.txt"), "schedule_set_text", { path: "s.txt" }],
    ["setText null", () => schedule.setText(null), "schedule_set_text", { path: null }],
    ["setXml", () => schedule.setXml("s.xml"), "schedule_set_xml", { path: "s.xml" }],
    ["setXml null", () => schedule.setXml(null), "schedule_set_xml", { path: null }],
    ["setDropped", () => schedule.setDropped(["a", "b"]), "schedule_set_dropped", { paths: ["a", "b"] }],
    [
      "apply",
      () => schedule.apply("a.txt", "b.xml"),
      "schedule_apply",
      { expectedText: "a.txt", expectedXml: "b.xml" },
    ],
  ])("%s invokes the right command", async (_name, run, command, args) => {
    await run();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith(command, args);
  });
});
