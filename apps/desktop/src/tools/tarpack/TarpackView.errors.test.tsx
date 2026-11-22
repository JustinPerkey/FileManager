import { createRef } from "react";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { TarpackView, type TarpackActions } from "./TarpackView";
import { diag, failure, manifest, session } from "./fixtures";

vi.mock("../../lib/tarpack", () => ({
  session: vi.fn(),
  openManifest: vi.fn(),
  reloadManifest: vi.fn(),
  openInEditor: vi.fn(),
  recentManifests: vi.fn(),
  createManifestFromExample: vi.fn(),
  onManifestChanged: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({ openFileDialog: vi.fn(), saveFileDialog: vi.fn(), onDragDrop: vi.fn() }));
import * as tp from "../../lib/tarpack";
import { openFileDialog, onDragDrop } from "../../lib/tauri";

let changed: () => void = () => undefined;

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(onDragDrop).mockResolvedValue(() => undefined);
  vi.mocked(tp.recentManifests).mockResolvedValue([]);
  vi.mocked(tp.onManifestChanged).mockImplementation(async (h) => {
    changed = () => h({ path: "x" });
    return () => undefined;
  });
});

const withheld = session(manifest({ entriesWithheld: true, entries: [], errors: [diag("a"), diag("b")] }));
const failed = session(manifest({ failedEntries: [failure(1), failure(2)] }));
const nameOnly = session(manifest({ errors: [diag("name bad")] }));
const clean = session(manifest());

async function mount(initial = clean) {
  vi.mocked(tp.session).mockResolvedValue(initial);
  const actions = createRef<TarpackActions>();
  const utils = render(<TarpackView actionsRef={actions} />);
  await screen.findByRole("heading", { level: 1 });
  return { ...utils, actions };
}
const announcer = () => screen.getByTestId("tarpack-announcer");

test.each([
  ["withheld", withheld, "gateway has 2 errors. No files can be built until the manifest errors are fixed."],
  ["failed entries", failed, "gateway has 2 errors. 2 files will be left out of the archive."],
  ["manifest-level only", nameOnly, "gateway has 1 error."],
])("restore announces %s", async (_n, s, text) => {
  await mount(s);
  await waitFor(() => expect(announcer()).toHaveTextContent(text));
});

test("open and reload announce, repeat on same count, and clear announces No errors", async () => {
  const user = userEvent.setup();
  await mount();
  expect(announcer()).toHaveTextContent("");
  vi.mocked(openFileDialog).mockResolvedValue("C:\\m.toml");
  vi.mocked(tp.openManifest).mockResolvedValue(failed);
  await user.click(screen.getByRole("button", { name: "Open…" }));
  await waitFor(() => expect(announcer()).toHaveTextContent("2 files will be left out"));

  vi.mocked(tp.reloadManifest).mockResolvedValue(failed);
  const seen: string[] = [];
  const obs = new MutationObserver(() => seen.push(announcer().textContent ?? ""));
  obs.observe(announcer(), { childList: true, characterData: true, subtree: true });
  await user.click(screen.getByRole("button", { name: "Reload" }));
  await waitFor(() => expect(announcer()).toHaveTextContent("2 files will be left out"));
  obs.disconnect();
  expect(seen).toContain(""); // cleared, then set again

  vi.mocked(tp.reloadManifest).mockResolvedValue(clean);
  await user.click(screen.getByRole("button", { name: "Reload" }));
  await waitFor(() => expect(announcer()).toHaveTextContent("gateway reloaded. No errors."));
  expect(screen.queryByRole("region")).toBeNull();
});

test("a repeated sentence is re-announced even when the frame fires before React renders", async () => {
  await mount(failed);
  await waitFor(() => expect(announcer()).toHaveTextContent("2 files will be left out"));
  vi.mocked(tp.reloadManifest).mockResolvedValue(failed);
  const g = globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean };
  const prevAct = g.IS_REACT_ACT_ENVIRONMENT;
  const realRaf = globalThis.requestAnimationFrame;
  const realCaf = globalThis.cancelAnimationFrame;
  // The frame fires in a microtask: after the announce() call, before React's scheduler (a macrotask) renders.
  // Stub ids start far above jsdom's, so a cancel of a real frame never hits a stub frame.
  const pending = new Map<number, FrameRequestCallback>();
  let next = 1_000_000;
  globalThis.requestAnimationFrame = (cb) => {
    const id = next++;
    pending.set(id, cb);
    queueMicrotask(() => {
      const fn = pending.get(id);
      pending.delete(id);
      fn?.(0);
    });
    return id;
  };
  globalThis.cancelAnimationFrame = (id) => {
    if (pending.has(id)) pending.delete(id);
    else realCaf.call(globalThis, id);
  };
  const seen: string[] = [];
  const obs = new MutationObserver(() => seen.push(announcer().textContent ?? ""));
  try {
    g.IS_REACT_ACT_ENVIRONMENT = false;
    obs.observe(announcer(), { childList: true, characterData: true, subtree: true });
    const reload = screen.getByRole("button", { name: "Reload" });
    reload.click();
    await waitFor(() => expect(tp.reloadManifest).toHaveBeenCalled());
    // Settle: the command resolves, React renders, the frames fire (all within a few ticks).
    await new Promise((r) => setTimeout(r, 50));
    obs.disconnect();
    expect(seen).toContain("");
    expect(seen[seen.length - 1]).toContain("2 files will be left out");
    expect(seen.indexOf("")).toBeLessThan(seen.length - 1);
  } finally {
    obs.disconnect();
    g.IS_REACT_ACT_ENVIRONMENT = prevAct;
    globalThis.requestAnimationFrame = realRaf;
    globalThis.cancelAnimationFrame = realCaf;
  }
});

test("report re-expands after reload even if collapsed; no focus stolen", async () => {
  const user = userEvent.setup();
  await mount(failed);
  await user.click(screen.getByRole("button", { name: "Hide errors" }));
  // The build bar also has a "Show errors" button; the report's own toggle is the one with aria-expanded.
  const toggles = screen.getAllByRole("button", { name: "Show errors" });
  expect(toggles.filter((b) => b.getAttribute("aria-expanded") === "false")).toHaveLength(1);
  vi.mocked(tp.reloadManifest).mockResolvedValue(failed);
  const reload = screen.getByRole("button", { name: "Reload" });
  reload.focus();
  await user.click(reload);
  expect(await screen.findByRole("button", { name: "Hide errors" })).toHaveAttribute("aria-expanded", "true");
  expect(reload).toHaveFocus();
});

test("showErrors expands and focuses the report body; no-op without errors", async () => {
  const user = userEvent.setup();
  const { actions } = await mount(failed);
  await user.click(screen.getByRole("button", { name: "Hide errors" }));
  act(() => actions.current!.showErrors());
  expect(screen.getByRole("region", { name: /errors in this manifest/ })).toHaveFocus();
  expect(screen.getByRole("button", { name: "Hide errors" })).toBeInTheDocument();
});

test("showErrors does nothing with zero errors", async () => {
  const { actions } = await mount(clean);
  const before = document.activeElement;
  act(() => actions.current!.showErrors());
  expect(document.activeElement).toBe(before);
});

test("announce() sets the announcer text", async () => {
  const { actions } = await mount(clean);
  act(() => actions.current!.announce("Built."));
  await waitFor(() => expect(announcer()).toHaveTextContent("Built."));
});

test("changed-on-disk is announced once, not again on a second event", async () => {
  await mount(clean);
  act(() => changed());
  await waitFor(() => expect(announcer()).toHaveTextContent("The manifest changed on disk."));
  const seen: string[] = [];
  const obs = new MutationObserver(() => seen.push(announcer().textContent ?? ""));
  obs.observe(announcer(), { childList: true, characterData: true, subtree: true });
  act(() => changed());
  await new Promise((r) => setTimeout(r, 50));
  obs.disconnect();
  expect(seen).toEqual([]);
});

test("a restore with a state warning and errors makes one combined announcement", async () => {
  await mount(session(failed.manifest, { stateWarning: "Saved locations were reset." }));
  await waitFor(() =>
    expect(announcer()).toHaveTextContent(
      "Saved locations were reset. gateway has 2 errors. 2 files will be left out of the archive.",
    ),
  );
});

test("no focus change on open or reload with errors", async () => {
  const user = userEvent.setup();
  await mount(clean);
  vi.mocked(openFileDialog).mockResolvedValue("C:\\m.toml");
  vi.mocked(tp.openManifest).mockResolvedValue(failed);
  const open = screen.getByRole("button", { name: "Open…" });
  open.focus();
  await user.click(open);
  await screen.findByRole("region", { name: /errors in this manifest/ });
  expect(open).toHaveFocus();
  vi.mocked(tp.reloadManifest).mockResolvedValue(failed);
  const reload = screen.getByRole("button", { name: "Reload" });
  await user.click(reload);
  await waitFor(() => expect(announcer()).toHaveTextContent("2 files will be left out"));
  expect(reload).toHaveFocus();
  expect(screen.getByRole("region", { name: /errors in this manifest/ })).not.toHaveFocus();
});

test("200 failed entries: all render, the report is the only tab stop and scrolls", async () => {
  const many = session(manifest({ failedEntries: Array.from({ length: 200 }, (_, i) => failure(i + 1)) }));
  const { container } = await mount(many);
  expect(screen.getAllByRole("heading", { level: 4 })).toHaveLength(200);
  const report = container.querySelector(".error-region__report")!;
  expect(report).toHaveAttribute("tabindex", "0");
  expect(container.querySelector(".error-region")!.querySelectorAll("[tabindex='0']")).toHaveLength(1);
  const buttonsInReport = report.querySelectorAll("button, a, [tabindex]:not([tabindex='-1'])");
  expect(buttonsInReport).toHaveLength(0);
});
