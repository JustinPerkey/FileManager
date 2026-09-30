import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { beforeEach, expect, test, vi } from "vitest";
import type { DropOutcome } from "../../lib/generated/DropOutcome";
import { TarpackView } from "./TarpackView";
import { dropResultText } from "./DropResult";
import { entry, failure, manifest, session } from "./fixtures";

vi.mock("../../lib/tarpack", () => ({
  session: vi.fn(),
  assign: vi.fn(),
  assignDropped: vi.fn(),
  clear: vi.fn(),
  recentManifests: vi.fn(),
  onManifestChanged: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({
  openFileDialog: vi.fn(),
  saveFileDialog: vi.fn(),
  onDragDrop: vi.fn(),
}));
import * as tp from "../../lib/tarpack";
import { onDragDrop, openFileDialog, type DragDrop } from "../../lib/tauri";

let emit: (e: DragDrop) => void = () => undefined;

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(tp.recentManifests).mockResolvedValue([]);
  vi.mocked(tp.onManifestChanged).mockResolvedValue(() => undefined);
  vi.mocked(onDragDrop).mockImplementation(async (h) => {
    emit = h;
    return () => undefined;
  });
});

const announcer = () => screen.getAllByRole("status")[0];
const fire = (type: DragDrop["type"], paths: string[] = []) => act(() => emit({ type, paths }));
const assigned = (id: string, path: string) => ({
  ...entry(id),
  assigned: path,
  status: "ready" as const,
});

const mount = async (
  s = session(
    manifest({
      entries: [entry("app"), assigned("core", "C:\\libs\\core.bin")],
    }),
  ),
) => {
  vi.mocked(tp.session).mockResolvedValue(s);
  const utils = render(<TarpackView />);
  await screen.findByRole("heading", { level: 1 });
  await waitFor(() => expect(onDragDrop).toHaveBeenCalled());
  await act(async () => undefined);
  return utils;
};
const drop = async (paths = ["C:\\d\\app.bin"]) => {
  fire("enter", paths);
  fire("drop", paths);
  await screen.findByRole("region", { name: "Drop result" });
};

const withErrors = () =>
  session(
    manifest({
      entries: [entry("app"), assigned("core", "C:\\libs\\core.bin")],
      failedEntries: [failure(1)],
    }),
  );
const outcome: DropOutcome = {
  matched: [["app", "C:\\d\\app.bin"]],
  unmatched: [{ path: "C:\\d\\notes.txt", reason: "noEntry" }],
  ambiguous: [],
};

test("drop calls assignDropped with the dropped paths, renders session and outcome, announces the result", async () => {
  const next = session(manifest({ entries: [assigned("app", "C:\\d\\app.bin"), entry("core")] }));
  vi.mocked(tp.assignDropped).mockResolvedValue({ session: next, outcome });
  const { container } = await mount();
  await drop();
  expect(tp.assignDropped).toHaveBeenCalledWith(["C:\\d\\app.bin"]);
  expect(screen.getByText("C:\\d\\app.bin", { exact: false, selector: "td *" })).toBeInTheDocument();
  expect(screen.getByRole("region", { name: "Drop result" })).toHaveTextContent("1 matched");
  await waitFor(() =>
    expect(announcer()).toHaveTextContent(dropResultText(outcome, next.manifest!.entries, false)),
  );
  expect(screen.queryByRole("region", { name: "Drop result" })!.closest("[aria-live]")).toBeNull();
  await user().click(screen.getByRole("button", { name: "Dismiss drop result" }));
  expect(screen.queryByRole("region", { name: "Drop result" })).toBeNull();
  expect(screen.getByRole("button", { name: "Browse… for /opt/app" })).toHaveFocus();
  expect(container.querySelector(".tarpack")).not.toHaveAttribute("tabindex");
  expect((await axe(container)).violations).toEqual([]);
});
const user = () => userEvent.setup();

test("overlay axe check while visible", async () => {
  const { container } = await mount();
  fire("enter", ["C:\\x"]);
  expect(screen.getByText("Drop files or folders to match them to the manifest")).toBeInTheDocument();
  expect((await axe(container)).violations).toEqual([]);
});

test("Browse assigns the chosen file, starting in the current folder", async () => {
  const next = session(manifest({ entries: [assigned("app", "C:\\p\\app.bin"), entry("core")] }));
  vi.mocked(openFileDialog).mockResolvedValue("C:\\p\\app.bin");
  vi.mocked(tp.assign).mockResolvedValue(next);
  await mount();
  await user().click(screen.getByRole("button", { name: "Browse… for /opt/core" }));
  expect(openFileDialog).toHaveBeenCalledWith({ defaultPath: "C:\\libs" });
  expect(tp.assign).toHaveBeenCalledWith("core", "C:\\p\\app.bin");
  expect(await screen.findByText("C:\\p\\app.bin", { exact: false, selector: "td *" })).toBeInTheDocument();
  expect(screen.queryByText("C:\\libs\\core.bin", { exact: false })).toBeNull();
});

test("Browse on an unassigned row has no default path; cancel makes no call", async () => {
  vi.mocked(openFileDialog).mockResolvedValue(null);
  await mount();
  await user().click(screen.getByRole("button", { name: "Browse… for /opt/app" }));
  expect(openFileDialog).toHaveBeenCalledWith({ defaultPath: undefined });
  expect(tp.assign).not.toHaveBeenCalled();
});

test("Clear calls clear with the id and keeps its title", async () => {
  vi.mocked(tp.clear).mockResolvedValue(session(manifest({ entries: [entry("app"), entry("core")] })));
  await mount();
  const btn = screen.getByRole("button", {
    name: "Clear assigned file for /opt/core",
  });
  expect(btn).toHaveAttribute("title", "Forget this file (nothing is deleted)");
  await user().click(btn);
  expect(tp.clear).toHaveBeenCalledWith("core");
});

test("a NotAFile rejection shows the copy and leaves the table unchanged", async () => {
  vi.mocked(openFileDialog).mockResolvedValue("C:\\dir");
  vi.mocked(tp.assign).mockRejectedValue({
    kind: "NotAFile",
    message: "C:\\dir is a dir",
    entryId: "app",
  });
  await mount();
  await user().click(screen.getByRole("button", { name: "Browse… for /opt/app" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("app.bin: the chosen path is not a file.");
  expect(screen.getByText("C:\\libs\\core.bin", { exact: false })).toBeInTheDocument();
});

test("a non-TarpackError rejection from assign shows the Io copy", async () => {
  vi.mocked(openFileDialog).mockResolvedValue("C:\\x");
  vi.mocked(tp.assign).mockRejectedValue(new Error("boom"));
  await mount();
  await user().click(screen.getByRole("button", { name: "Browse… for /opt/app" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("A file could not be read or written.");
});

test("a rejected drop shows an error banner and no result", async () => {
  vi.mocked(tp.assignDropped).mockRejectedValue({
    kind: "NoManifest",
    message: "none",
  });
  await mount();
  fire("drop", ["C:\\x"]);
  expect(await screen.findByRole("alert")).toHaveTextContent("Open a manifest first.");
  expect(screen.queryByRole("region", { name: "Drop result" })).toBeNull();
});

test("with errorCount > 0 and passed entries, drop and Browse work", async () => {
  const s = withErrors();
  expect(s.manifest!.errorCount).toBeGreaterThan(0);
  vi.mocked(tp.assignDropped).mockResolvedValue({ session: s, outcome });
  vi.mocked(openFileDialog).mockResolvedValue("C:\\p\\a.bin");
  vi.mocked(tp.assign).mockResolvedValue(s);
  await mount(s);
  await drop();
  expect(tp.assignDropped).toHaveBeenCalled();
  expect(screen.getByRole("region", { name: "Drop result" })).toHaveTextContent("1 not matched: notes.txt");
  expect(screen.getByRole("region", { name: "Drop result" })).toHaveTextContent("can't be matched until");
  await user().click(screen.getByRole("button", { name: "Browse… for /opt/app" }));
  expect(tp.assign).toHaveBeenCalledWith("app", "C:\\p\\a.bin");
});

test("silence on assign: Browse and Clear leave the announcer alone; a drop announces only its result", async () => {
  const s = withErrors();
  vi.mocked(openFileDialog).mockResolvedValue("C:\\p\\a.bin");
  vi.mocked(tp.assign).mockResolvedValue(s);
  vi.mocked(tp.clear).mockResolvedValue(s);
  vi.mocked(tp.assignDropped).mockResolvedValue({ session: s, outcome });
  await mount(s);
  await waitFor(() => expect(announcer()).toHaveTextContent("has 1 error"));
  await act(async () => new Promise((r) => setTimeout(r, 30)));
  const u = user();
  await u.click(screen.getByRole("button", { name: "Browse… for /opt/app" }));
  await u.click(screen.getByRole("button", { name: "Clear assigned file for /opt/core" }));
  await act(async () => new Promise((r) => setTimeout(r, 30)));
  expect(announcer()).toHaveTextContent("has 1 error");
  await drop();
  await waitFor(() =>
    expect(announcer().textContent).toBe(dropResultText(outcome, s.manifest!.entries, true)),
  );
  expect(announcer().textContent).not.toMatch(/has \d+ errors?/);
});

test.each([
  ["no manifest", session(null), "Open a manifest first"],
  [
    "withheld",
    session(
      manifest({
        entries: [],
        entriesWithheld: true,
        errors: [{ severity: "error", line: 1, col: 1, entryId: null, message: "x" }],
      }),
    ),
    "Fix the manifest errors first. No files can be matched yet.",
  ],
  [
    "every entry failed",
    session(manifest({ entries: [], failedEntries: [failure(1)] })),
    "Fix the manifest errors first. No files can be matched yet.",
  ],
  ["no files", session(manifest({ entries: [] })), "This manifest lists no files"],
])("disabled (%s): overlay shows the reason and a drop makes no call", async (_n, s, reason) => {
  await mount(s);
  fire("enter", ["C:\\x"]);
  expect(screen.getByText(reason)).toBeInTheDocument();
  fire("drop", ["C:\\x"]);
  expect(tp.assignDropped).not.toHaveBeenCalled();
  expect(screen.queryByRole("region", { name: "Drop result" })).toBeNull();
});

test("building disables drops", async () => {
  vi.mocked(tp.session).mockResolvedValue(session(manifest()));
  render(<TarpackView building />);
  await screen.findByRole("heading", { level: 1 });
  await waitFor(() => expect(onDragDrop).toHaveBeenCalled());
  await act(async () => undefined);
  fire("enter", ["C:\\x"]);
  expect(screen.getByText("A build is running")).toBeInTheDocument();
  fire("drop", ["C:\\x"]);
  expect(tp.assignDropped).not.toHaveBeenCalled();
});

const tick = (ms: number) => act(async () => void (await vi.advanceTimersByTimeAsync(ms)));

test("focus after Dismiss goes to the first tabbable after the result (the build bar)", async () => {
  const empty = session(manifest({ entries: [], failedEntries: [failure(1)] }));
  vi.mocked(tp.assignDropped).mockResolvedValue({ session: empty, outcome });
  await mount();
  await drop();
  screen.getByRole("button", { name: "Dismiss drop result" }).focus();
  await userEvent.keyboard("{Enter}");
  expect(screen.queryByRole("region", { name: "Drop result" })).toBeNull();
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Choose…" }));
  expect(document.querySelector(".tarpack")).not.toHaveAttribute("tabindex");
});

test("one drop at a time: pending state, disabled reason, 150 ms line, 1 s announcement, resolve", async () => {
  let resolve!: (v: { session: ReturnType<typeof session>; outcome: DropOutcome }) => void;
  vi.mocked(tp.assignDropped).mockReturnValue(new Promise((r) => (resolve = r)));
  const s = session(manifest({ entries: [entry("app"), assigned("core", "C:\\libs\\core.bin")] }));
  await mount(s);
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
  fire("drop", ["C:\\a.bin"]);
  expect(tp.assignDropped).toHaveBeenCalledTimes(1);
  fire("drop", ["C:\\b.bin"]);
  expect(tp.assignDropped).toHaveBeenCalledTimes(1);
  fire("enter", ["C:\\b.bin"]);
  expect(screen.getByText("Still matching the last drop")).toBeInTheDocument();
  fire("leave");
  expect(screen.queryByText("Matching dropped files…")).toBeNull();
  await tick(150);
  expect(screen.getByText("Matching dropped files…")).toBeInTheDocument();
  await tick(900);
  vi.useRealTimers();
  await waitFor(() => expect(announcer()).toHaveTextContent("Matching dropped files…"));
  await act(async () => resolve({ session: s, outcome }));
  expect(await screen.findByRole("region", { name: "Drop result" })).toBeInTheDocument();
  expect(screen.queryByText("Matching dropped files…")).toBeNull();
  fire("enter", ["C:\\c.bin"]);
  expect(screen.getByText("Drop files or folders to match them to the manifest")).toBeInTheDocument();
});

test("a rejected drop clears the previous result and shows the banner", async () => {
  vi.mocked(tp.assignDropped).mockResolvedValueOnce({ session: withErrors(), outcome });
  await mount();
  await drop();
  vi.mocked(tp.assignDropped).mockRejectedValueOnce({ kind: "Io", message: "changed" });
  fire("drop", ["C:\\again.bin"]);
  expect(await screen.findByRole("alert")).toHaveTextContent("A file could not be read or written.");
  expect(screen.queryByRole("region", { name: "Drop result" })).toBeNull();
});

test("a result keeps its hasFailedEntries when the session later changes", async () => {
  const clean = session(manifest({ entries: [entry("app"), assigned("core", "C:\\libs\\core.bin")] }));
  vi.mocked(tp.assignDropped).mockResolvedValue({ session: withErrors(), outcome });
  vi.mocked(tp.clear).mockResolvedValue(clean);
  await mount(withErrors());
  await drop();
  expect(screen.getByRole("region", { name: "Drop result" })).toHaveTextContent("1 not matched: notes.txt");
  await user().click(screen.getByRole("button", { name: "Clear assigned file for /opt/core" }));
  await waitFor(() => expect(tp.clear).toHaveBeenCalled());
  await act(async () => undefined);
  expect(screen.getByRole("region", { name: "Drop result" })).toHaveTextContent("1 not matched: notes.txt");
});

test("view-level axe with the result visible and Full paths expanded", async () => {
  const o: DropOutcome = {
    ...outcome,
    ambiguous: [{ id: "app", candidates: ["C:\\a\\app.bin", "C:\\b\\app.bin"] }],
  };
  vi.mocked(tp.assignDropped).mockResolvedValue({ session: withErrors(), outcome: o });
  const { container } = await mount(withErrors());
  await drop();
  await user().click(screen.getByText("Full paths"));
  expect(container.querySelector("details.drop-result__paths")).toHaveAttribute("open");
  expect((await axe(container)).violations).toEqual([]);
});
