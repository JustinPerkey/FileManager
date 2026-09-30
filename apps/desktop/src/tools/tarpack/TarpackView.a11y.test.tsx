import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { beforeEach, expect, test, vi } from "vitest";
import type { Progress } from "../../lib/generated/Progress";
import type { TarpackSession } from "../../lib/generated/TarpackSession";
import { TarpackView } from "./TarpackView";
import { buildable, diag, entry, failure, manifest, session, summary } from "./fixtures";

vi.mock("../../lib/tarpack", () => ({
  session: vi.fn(),
  build: vi.fn(),
  setOutput: vi.fn(),
  setFormat: vi.fn(),
  revealOutput: vi.fn(),
  reloadManifest: vi.fn(),
  recentManifests: vi.fn(),
  onManifestChanged: vi.fn(),
  onBuildProgress: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({ openFileDialog: vi.fn(), saveFileDialog: vi.fn(), onDragDrop: vi.fn() }));
import * as tp from "../../lib/tarpack";
import { onDragDrop } from "../../lib/tauri";

let emit: (p: Progress) => void = () => undefined;

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(onDragDrop).mockResolvedValue(() => undefined);
  vi.mocked(tp.recentManifests).mockResolvedValue([]);
  vi.mocked(tp.onManifestChanged).mockResolvedValue(() => undefined);
  vi.mocked(tp.onBuildProgress).mockImplementation(async (h) => {
    emit = h;
    return () => undefined;
  });
});

async function mount(s: TarpackSession) {
  vi.mocked(tp.session).mockResolvedValue(s);
  const utils = render(<TarpackView />);
  await screen.findByRole("heading", { level: 1 });
  return utils;
}
const clean = async (container: HTMLElement) => expect((await axe(container)).violations).toEqual([]);
const files = [entry("gateway", "gateway.conf"), entry("core", "core.bin")];
const create = () => screen.getByRole("button", { name: "Create archive" });

test("no manifest", async () => {
  const { container } = await mount(session(null));
  await clean(container);
});

test("errors with entries shown: report expanded, then collapsed", async () => {
  const user = userEvent.setup();
  const { container } = await mount(buildable(manifest({ entries: files, failedEntries: [failure(1)] })));
  await clean(container);
  await user.click(screen.getByRole("button", { name: "Hide errors" }));
  await clean(container);
});

test("errors with entries withheld", async () => {
  const { container } = await mount(
    session(manifest({ entriesWithheld: true, entries: [], errors: [diag("bad toml")] }), {
      buildBlockedReason: "noEntries",
    }),
  );
  await clean(container);
});

test("partial and ready", async () => {
  const partial = buildable(manifest({ entries: files }), {
    canBuild: false,
    readyCount: 1,
    buildBlockedReason: "entriesNotReady",
  });
  const { container, unmount } = await mount(partial);
  await clean(container);
  unmount();
  const r = await mount(buildable(manifest({ entries: files })));
  await clean(r.container);
});

test("ready with files left out", async () => {
  const { container } = await mount(buildable(manifest({ entries: files, failedEntries: [failure(1)] })));
  expect(document.querySelector(".build-bar__note")).toHaveTextContent(/will be left out/);
  await clean(container);
});

test("building: writing, then verifying", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockReturnValue(new Promise(() => undefined));
  const { container } = await mount(buildable(manifest({ entries: files })));
  await user.click(create());
  act(() => emit({ phase: "writing", entryId: "gateway", bytesDone: 40, bytesTotal: 100 }));
  await clean(container);
  act(() => emit({ phase: "verifying", entryId: null, bytesDone: 60, bytesTotal: 100 }));
  await clean(container);
});

test("success with the extraction command and normalised entries", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockResolvedValue(summary({ normalizedEntries: [{ id: "gateway", crlfReplaced: 3 }] }));
  const { container } = await mount(buildable(manifest({ entries: files })));
  await user.click(create());
  await screen.findByRole("heading", { level: 2, name: "Created gateway.tar.zst" });
  await clean(container);
});

test("success with files left out, with the report", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockResolvedValue(
    summary({ leftOut: [failure(1)], errorCount: 1, builtIds: ["gateway"], warnings: [diag("long name")] }),
  );
  const { container } = await mount(buildable(manifest({ entries: files, failedEntries: [failure(1)] })));
  await user.click(create());
  await screen.findByRole("heading", { level: 2, name: /left out/ });
  await clean(container);
});

test("build error", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockRejectedValue({ kind: "Io", message: "disk full" });
  const { container } = await mount(buildable(manifest({ entries: files })));
  await user.click(create());
  await screen.findByRole("heading", { level: 2 });
  await clean(container);
});

test("shortcuts popover open", async () => {
  const user = userEvent.setup();
  const { container } = await mount(buildable(manifest({ entries: files })));
  await user.click(screen.getByRole("button", { name: "Keyboard shortcuts" }));
  await clean(container);
});
