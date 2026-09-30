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
  await waitFor(() => expect(announcer()).toHaveTextContent(dropResultText(outcome, false)));
  expect(screen.queryByRole("region", { name: "Drop result" })!.closest("[aria-live]")).toBeNull();
  await user().click(screen.getByRole("button", { name: "Dismiss drop result" }));
  expect(screen.queryByRole("region", { name: "Drop result" })).toBeNull();
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
  await waitFor(() => expect(announcer().textContent).toBe(dropResultText(outcome, true)));
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
