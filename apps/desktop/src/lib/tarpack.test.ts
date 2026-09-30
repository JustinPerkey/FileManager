import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const listen = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: (...a: unknown[]) => listen(...a) }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

import * as tarpack from "./tarpack";

beforeEach(() => {
  vi.clearAllMocks();
  invoke.mockResolvedValue(undefined);
});

describe("tarpack client", () => {
  it.each([
    ["session", () => tarpack.session(), "tarpack_session", undefined],
    ["openManifest", () => tarpack.openManifest("m.toml"), "tarpack_open_manifest", { path: "m.toml" }],
    ["reloadManifest", () => tarpack.reloadManifest(), "tarpack_reload_manifest", undefined],
    ["assignDropped", () => tarpack.assignDropped(["a", "b"]), "tarpack_assign_dropped", { paths: ["a", "b"] }],
    ["assign", () => tarpack.assign("id", "f"), "tarpack_assign", { id: "id", path: "f" }],
    ["clear", () => tarpack.clear("id"), "tarpack_clear", { id: "id" }],
    ["setOutput", () => tarpack.setOutput("o.tar"), "tarpack_set_output", { path: "o.tar" }],
    ["setFormat", () => tarpack.setFormat("tarZst"), "tarpack_set_format", { format: "tarZst" }],
    ["build", () => tarpack.build(true), "tarpack_build", { overwrite: true }],
    ["recentManifests", () => tarpack.recentManifests(), "tarpack_recent_manifests", undefined],
    ["openInEditor", () => tarpack.openInEditor(), "tarpack_open_in_editor", undefined],
    ["revealOutput", () => tarpack.revealOutput(), "tarpack_reveal_output", undefined],
    [
      "createManifestFromExample",
      () => tarpack.createManifestFromExample("n.toml"),
      "tarpack_create_manifest_from_example",
      { path: "n.toml" },
    ],
  ])("%s invokes the right command", async (_name, run, command, args) => {
    await run();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith(command, args);
  });

  it("revealOutput passes no arguments", async () => {
    await tarpack.revealOutput();
    expect(invoke.mock.calls[0]).toEqual(["tarpack_reveal_output", undefined]);
  });

  it("subscribes to the two events", async () => {
    listen.mockResolvedValue(() => {});
    await tarpack.onManifestChanged(() => {});
    await tarpack.onBuildProgress(() => {});
    expect(listen.mock.calls.map((c) => c[0])).toEqual([
      "tarpack://manifest-changed",
      "tarpack://build-progress",
    ]);
  });
});
