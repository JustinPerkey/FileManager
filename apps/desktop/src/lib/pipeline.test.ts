import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => invoke(...a),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

import * as pipeline from "./pipeline";

beforeEach(() => {
  vi.clearAllMocks();
  invoke.mockResolvedValue(undefined);
});

describe("pipeline client", () => {
  it.each([
    ["session", () => pipeline.session(), "pipeline_session", undefined],
    ["setDirectory", () => pipeline.setDirectory("C:\\jobs"), "pipeline_set_directory", { path: "C:\\jobs" }],
    ["setDirectory null", () => pipeline.setDirectory(null), "pipeline_set_directory", { path: null }],
    [
      "runStep",
      () => pipeline.runStep("RunBatch", "C:\\jobs"),
      "pipeline_run_step",
      { step: "RunBatch", expectedDirectory: "C:\\jobs" },
    ],
  ])("%s invokes the right command", async (_name, run, command, args) => {
    await run();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith(command, args);
  });
});
