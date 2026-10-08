import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { beforeEach, expect, test, vi } from "vitest";
import type { ScheduleSession } from "../../lib/generated/ScheduleSession";
import { cleanPath, ScheduleView } from "./ScheduleView";

vi.mock("../../lib/schedule", () => ({
  session: vi.fn(),
  setText: vi.fn(),
  setXml: vi.fn(),
  setDropped: vi.fn(),
  apply: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({ openFileDialog: vi.fn(), onDragDrop: vi.fn() }));
import * as sc from "../../lib/schedule";
import { onDragDrop, openFileDialog, type DragDrop } from "../../lib/tauri";

const TXT = "C:\\work\\week.txt";
const XML = "C:\\work\\schedule.xml";
const empty: ScheduleSession = { text: null, xml: null, canApply: false };
const ready: ScheduleSession = {
  text: { path: TXT, error: null },
  xml: { path: XML, error: null },
  canApply: true,
};

let drag: (e: DragDrop) => void = () => undefined;

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(onDragDrop).mockImplementation(async (h) => {
    drag = h;
    return () => undefined;
  });
});

const load = async (s: ScheduleSession) => {
  vi.mocked(sc.session).mockResolvedValue(s);
  const utils = render(<ScheduleView />);
  await screen.findByRole("textbox", { name: "Schedule file" });
  return utils;
};
const textBox = () => screen.getByRole("textbox", { name: "Schedule file" });
const xmlBox = () => screen.getByRole("textbox", { name: "XML file to update" });

test("cleanPath trims and drops Explorer's quotes", () => {
  expect(cleanPath('  "C:\\a b\\s.txt"  ')).toBe("C:\\a b\\s.txt");
  expect(cleanPath("   ")).toBeNull();
});

test("empty state: two labelled fields, Add disabled with a reason", async () => {
  const { container } = await load(empty);
  expect(screen.getByRole("heading", { level: 1, name: "Schedule Creator" })).toBeInTheDocument();
  expect(textBox()).toHaveValue("");
  expect(xmlBox()).toHaveValue("");
  const add = screen.getByRole("button", { name: "Add to XML…" });
  expect(add).toBeDisabled();
  expect(add).toHaveAccessibleDescription("Choose a schedule file.");
  expect((await axe(container)).violations).toEqual([]);
});

test("a typed path is set on Enter, cleaned of quotes", async () => {
  const user = userEvent.setup();
  await load(empty);
  vi.mocked(sc.setText).mockResolvedValue({ ...empty, text: { path: TXT, error: null } });
  await user.type(textBox(), `"${TXT}"{Enter}`);
  expect(sc.setText).toHaveBeenCalledWith(TXT);
  await waitFor(() => expect(textBox()).toHaveValue(TXT));
  expect(screen.getByRole("button", { name: "Add to XML…" })).toHaveAccessibleDescription(
    "Choose the XML file to update.",
  );
});

test("a typed path is set when leaving the field, and not resent unchanged", async () => {
  const user = userEvent.setup();
  await load(empty);
  vi.mocked(sc.setXml).mockResolvedValue({ ...empty, xml: { path: XML, error: null } });
  await user.type(xmlBox(), XML);
  await user.tab();
  expect(sc.setXml).toHaveBeenCalledWith(XML);
  await waitFor(() => expect(xmlBox()).toHaveValue(XML));
  await user.click(xmlBox());
  await user.tab();
  expect(sc.setXml).toHaveBeenCalledTimes(1);
});

test("emptying a field clears the file", async () => {
  const user = userEvent.setup();
  await load(ready);
  vi.mocked(sc.setText).mockResolvedValue({ ...ready, text: null, canApply: false });
  await user.clear(textBox());
  await user.keyboard("{Enter}");
  expect(sc.setText).toHaveBeenCalledWith(null);
});

test("Escape restores the field", async () => {
  const user = userEvent.setup();
  await load(ready);
  await user.type(textBox(), "zzz");
  await user.keyboard("{Escape}");
  expect(textBox()).toHaveValue(TXT);
});

test("a bad path stays in the field, marked invalid with its error", async () => {
  const { container } = await load({
    ...empty,
    text: { path: "C:\\nope.txt", error: { kind: "NotAFile", message: "C:\\nope.txt is not a file" } },
  });
  expect(textBox()).toHaveValue("C:\\nope.txt");
  expect(textBox()).toBeInvalid();
  expect(textBox()).toHaveAccessibleDescription("No file was found at this path.");
  expect((await axe(container)).violations).toEqual([]);
});

test("Browse opens the dialog with the right filters", async () => {
  const user = userEvent.setup();
  await load(empty);
  vi.mocked(openFileDialog).mockResolvedValueOnce(TXT).mockResolvedValueOnce(null);
  vi.mocked(sc.setText).mockResolvedValue({ ...empty, text: { path: TXT, error: null } });
  await user.click(screen.getByRole("button", { name: "Browse for the schedule file" }));
  expect(sc.setText).toHaveBeenCalledWith(TXT);
  await waitFor(() => expect(textBox()).toHaveValue(TXT));
  await user.click(screen.getByRole("button", { name: "Browse for the XML file" }));
  const filters = vi.mocked(openFileDialog).mock.calls.map((c) => c[0]?.filters?.map((f) => f.extensions));
  expect(filters).toEqual([
    [["txt"], ["*"]],
    [["xml"], ["*"]],
  ]);
  expect(sc.setXml).not.toHaveBeenCalled();
});

test("dropping files sets them and announces what was set", async () => {
  await load(empty);
  vi.mocked(sc.setDropped).mockResolvedValue(ready);
  await act(async () => drag({ type: "drop", paths: [XML, TXT] }));
  expect(sc.setDropped).toHaveBeenCalledWith([XML, TXT]);
  await waitFor(() => expect(xmlBox()).toHaveValue(XML));
  expect(textBox()).toHaveValue(TXT);
  await waitFor(() =>
    expect(screen.getByRole("status")).toHaveTextContent("Schedule file: week.txt. XML file: schedule.xml."),
  );
});

test("an ambiguous drop is reported", async () => {
  await load(empty);
  vi.mocked(sc.setDropped).mockRejectedValue({ kind: "DropAmbiguous", message: "2 XML" });
  await act(async () => drag({ type: "drop", paths: ["a.xml", "b.xml"] }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Drop one schedule file, one XML file, or one of each.",
  );
});

test("adding confirms first, then reports the backup and moves focus to it", async () => {
  const user = userEvent.setup();
  const { container } = await load(ready);
  await user.click(screen.getByRole("button", { name: "Add to XML…" }));
  const dialog = await screen.findByRole("dialog", { name: "Update schedule.xml?" });
  expect(dialog).toHaveTextContent(`The schedule in ${TXT} will be read and added to ${XML}`);
  expect(dialog).not.toHaveTextContent("already added");
  expect(sc.apply).not.toHaveBeenCalled();

  vi.mocked(sc.apply).mockResolvedValue({ xmlPath: XML, backupPath: `${XML}.bak`, added: 2 });
  await user.click(screen.getByRole("button", { name: "Update" }));
  expect(sc.apply).toHaveBeenCalledTimes(1);
  expect(sc.apply).toHaveBeenCalledWith(TXT, XML);
  await waitFor(() => expect(document.activeElement).toHaveTextContent("Added 2 entries to schedule.xml."));
  expect(screen.getByText(`${XML}.bak`)).toBeInTheDocument();
  expect((await axe(container)).violations).toEqual([]);

  await user.click(screen.getByRole("button", { name: "Add to XML…" }));
  expect(await screen.findByRole("dialog")).toHaveTextContent(
    "This schedule was already added to this file.",
  );
});

test("cancelling the confirmation writes nothing", async () => {
  const user = userEvent.setup();
  await load(ready);
  await user.click(screen.getByRole("button", { name: "Add to XML…" }));
  await user.click(await screen.findByRole("button", { name: "Cancel" }));
  expect(sc.apply).not.toHaveBeenCalled();
});

test("a parse error from Add names the line and takes focus", async () => {
  const user = userEvent.setup();
  await load(ready);
  vi.mocked(sc.apply).mockRejectedValue({ kind: "ParseFailed", message: "missing time", line: 4 });
  await user.click(screen.getByRole("button", { name: "Add to XML…" }));
  await user.click(await screen.findByRole("button", { name: "Update" }));
  await waitFor(() =>
    expect(document.activeElement).toHaveTextContent(
      "Line 4 of the schedule file: missing time. Nothing was changed.",
    ),
  );
});

test("the stub hooks say they are not implemented", async () => {
  const user = userEvent.setup();
  await load(ready);
  vi.mocked(sc.apply).mockRejectedValue({
    kind: "ParseNotImplemented",
    message: "schedule parsing is not implemented yet",
  });
  await user.click(screen.getByRole("button", { name: "Add to XML…" }));
  await user.click(await screen.findByRole("button", { name: "Update" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Reading schedule files is not implemented yet. Nothing was changed.",
  );
});

test("an unknown rejection claims nothing about the files", async () => {
  const user = userEvent.setup();
  await load(empty);
  vi.mocked(openFileDialog).mockResolvedValue(XML);
  vi.mocked(sc.setXml).mockRejectedValue(new Error("ipc down"));
  await user.click(screen.getByRole("button", { name: "Browse for the XML file" }));
  const alert = await screen.findByRole("alert");
  expect(alert).toHaveTextContent("Something went wrong. See Details.");
  expect(alert).toHaveTextContent("ipc down");
});

test("a failed first load can be retried", async () => {
  const user = userEvent.setup();
  vi.mocked(sc.session).mockRejectedValueOnce(new Error("ipc down")).mockResolvedValue(empty);
  render(<ScheduleView />);
  await user.click(await screen.findByRole("button", { name: "Try again" }));
  expect(await screen.findByRole("textbox", { name: "Schedule file" })).toBeInTheDocument();
  expect(screen.queryByRole("alert")).toBeNull();
});

test("drops are ignored while the confirmation is open", async () => {
  const user = userEvent.setup();
  await load(ready);
  await user.click(screen.getByRole("button", { name: "Add to XML…" }));
  await screen.findByRole("dialog");
  await act(async () => drag({ type: "drop", paths: ["C:\\other.xml"] }));
  expect(sc.setDropped).not.toHaveBeenCalled();
});

test("a late response from an older command is ignored", async () => {
  const user = userEvent.setup();
  await load(empty);
  let finishOld: (s: ScheduleSession) => void = () => undefined;
  vi.mocked(sc.setText)
    .mockReturnValueOnce(new Promise((r) => (finishOld = r)))
    .mockResolvedValue({ ...empty, text: { path: "C:\\old.txt", error: null } });
  await user.type(textBox(), "C:\\old.txt{Enter}");
  vi.mocked(sc.setXml).mockResolvedValue({ ...empty, xml: { path: XML, error: null } });
  await user.type(xmlBox(), `${XML}{Enter}`);
  await waitFor(() => expect(xmlBox()).toHaveValue(XML));
  await act(async () => finishOld({ ...empty, text: { path: "C:\\old.txt", error: null } }));
  expect(xmlBox()).toHaveValue(XML);
  expect(screen.getByRole("button", { name: "Add to XML…" })).toHaveAccessibleDescription(
    "Choose a schedule file.",
  );
});

test("a typed path's error is announced", async () => {
  const user = userEvent.setup();
  await load(empty);
  vi.mocked(sc.setText).mockResolvedValue({
    ...empty,
    text: { path: "week.txt", error: { kind: "NotAbsolute", message: "week.txt is not a full path" } },
  });
  await user.type(textBox(), "week.txt{Enter}");
  await waitFor(() =>
    expect(screen.getByRole("status")).toHaveTextContent(
      "Enter the full path, starting with the drive letter",
    ),
  );
});
