import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { beforeEach, expect, test, vi } from "vitest";
import type { ScheduleSession } from "../../lib/generated/ScheduleSession";
import { ScheduleView } from "./ScheduleView";

vi.mock("../../lib/schedule", () => ({
  session: vi.fn(),
  openText: vi.fn(),
  reloadText: vi.fn(),
  openXml: vi.fn(),
  apply: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({ openFileDialog: vi.fn() }));
import * as sc from "../../lib/schedule";
import { openFileDialog } from "../../lib/tauri";

const empty: ScheduleSession = {
  textPath: null,
  xmlPath: null,
  preview: null,
  parseError: null,
  canApply: false,
};
const preview = { columns: ["When", "What"], rows: [["09:00", "Standup"], ["13:00", "Review"]] };
const ready: ScheduleSession = {
  textPath: "C:\\work\\week.txt",
  xmlPath: "C:\\work\\schedule.xml",
  preview,
  parseError: null,
  canApply: true,
};

beforeEach(() => {
  vi.resetAllMocks();
});

const load = async (s: ScheduleSession) => {
  vi.mocked(sc.session).mockResolvedValue(s);
  const utils = render(<ScheduleView />);
  await screen.findByRole("button", { name: "Choose schedule file…" });
  return utils;
};

test("empty state explains why it cannot add yet", async () => {
  const { container } = await load(empty);
  expect(screen.getByRole("heading", { level: 1, name: "Schedule Creator" })).toBeInTheDocument();
  expect(screen.getAllByText("None chosen")).toHaveLength(2);
  const add = screen.getByRole("button", { name: "Add to XML…" });
  expect(add).toBeDisabled();
  expect(add).toHaveAccessibleDescription("Choose a schedule text file.");
  expect((await axe(container)).violations).toEqual([]);
});

test("choosing a text file opens it with a txt filter and shows the preview", async () => {
  const user = userEvent.setup();
  await load(empty);
  vi.mocked(openFileDialog).mockResolvedValue("C:\\work\\week.txt");
  vi.mocked(sc.openText).mockResolvedValue({ ...empty, textPath: "C:\\work\\week.txt", preview });
  await user.click(screen.getByRole("button", { name: "Choose schedule file…" }));
  expect(vi.mocked(openFileDialog).mock.calls[0][0]?.filters?.[0].extensions).toEqual(["txt"]);
  expect(sc.openText).toHaveBeenCalledWith("C:\\work\\week.txt");
  const table = await screen.findByRole("table", { name: "2 entries to add" });
  expect(table).toHaveTextContent("Standup");
  expect(screen.getByRole("columnheader", { name: "When" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Add to XML…" })).toHaveAccessibleDescription(
    "Choose the XML file to update.",
  );
});

test("a cancelled dialog does nothing", async () => {
  const user = userEvent.setup();
  await load(empty);
  vi.mocked(openFileDialog).mockResolvedValue(null);
  await user.click(screen.getByRole("button", { name: "Choose XML file…" }));
  expect(sc.openXml).not.toHaveBeenCalled();
});

test("the stub parser shows a not-implemented warning", async () => {
  await load({
    ...empty,
    textPath: "C:\\work\\week.txt",
    parseError: { kind: "ParseNotImplemented", message: "schedule parsing is not implemented yet" },
  });
  expect(screen.getByText("Reading schedule files is not implemented yet.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Reload" })).toBeInTheDocument();
});

test("a parse error names the line", async () => {
  await load({
    ...empty,
    textPath: "C:\\work\\week.txt",
    parseError: { kind: "ParseFailed", message: "missing time", line: 4 },
  });
  expect(screen.getByRole("alert")).toHaveTextContent("Line 4 of the schedule file: missing time");
});

test("reload re-reads the text file", async () => {
  const user = userEvent.setup();
  await load(ready);
  vi.mocked(sc.reloadText).mockResolvedValue(ready);
  await user.click(screen.getByRole("button", { name: "Reload" }));
  expect(sc.reloadText).toHaveBeenCalledTimes(1);
});

test("adding confirms first, then reports the backup", async () => {
  const user = userEvent.setup();
  const { container } = await load(ready);
  await user.click(screen.getByRole("button", { name: "Add to XML…" }));
  const dialog = await screen.findByRole("dialog", { name: "Update schedule.xml?" });
  expect(dialog).toHaveTextContent("backup copy");
  expect(sc.apply).not.toHaveBeenCalled();

  vi.mocked(sc.apply).mockResolvedValue({
    xmlPath: "C:\\work\\schedule.xml",
    backupPath: "C:\\work\\schedule.xml.bak",
    added: 2,
  });
  await user.click(screen.getByRole("button", { name: "Update" }));
  expect(sc.apply).toHaveBeenCalledTimes(1);
  expect(await screen.findByText("Added 2 entries to schedule.xml.")).toBeInTheDocument();
  expect(screen.getByText("C:\\work\\schedule.xml.bak")).toBeInTheDocument();
  expect((await axe(container)).violations).toEqual([]);
});

test("cancelling the confirmation writes nothing", async () => {
  const user = userEvent.setup();
  await load(ready);
  await user.click(screen.getByRole("button", { name: "Add to XML…" }));
  await user.click(await screen.findByRole("button", { name: "Cancel" }));
  expect(sc.apply).not.toHaveBeenCalled();
});

test("a failed update is reported with its detail", async () => {
  const user = userEvent.setup();
  await load(ready);
  vi.mocked(sc.apply).mockRejectedValue({
    kind: "MergeNotImplemented",
    message: "updating the XML is not implemented yet",
  });
  await user.click(screen.getByRole("button", { name: "Add to XML…" }));
  await user.click(await screen.findByRole("button", { name: "Update" }));
  await waitFor(() =>
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Updating the XML file is not implemented yet. Nothing was changed.",
    ),
  );
  expect(screen.getByText("updating the XML is not implemented yet")).toBeInTheDocument();
});

test("an unknown rejection becomes an Io error", async () => {
  const user = userEvent.setup();
  await load(empty);
  vi.mocked(openFileDialog).mockResolvedValue("C:\\x.xml");
  vi.mocked(sc.openXml).mockRejectedValue(new Error("ipc down"));
  await user.click(screen.getByRole("button", { name: "Choose XML file…" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("A file could not be written.");
});
