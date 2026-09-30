import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { EntryTable } from "./EntryTable";
import { TarpackView } from "./TarpackView";
import { entry, manifest, session } from "./fixtures";
import type { DropOutcome } from "../../lib/generated/DropOutcome";
import type { SessionEntry } from "../../lib/generated/SessionEntry";

vi.mock("../../lib/tarpack", () => ({
  session: vi.fn(),
  assignDropped: vi.fn(),
  recentManifests: vi.fn(),
  onManifestChanged: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({ openFileDialog: vi.fn(), saveFileDialog: vi.fn(), onDragDrop: vi.fn() }));
import * as tp from "../../lib/tarpack";
import { onDragDrop, type DragDrop } from "../../lib/tauri";

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

const e = (id: string, over: Partial<SessionEntry> = {}): SessionEntry => ({ ...entry(id), ...over });
const entries = [
  e("a", { status: "ready", assigned: "C:\\src\\a.bin" }),
  e("b"),
  e("c", { status: "ready", assigned: "C:\\src\\c.bin" }),
];

function setup(disabled?: boolean) {
  const onBrowse = vi.fn();
  const onClear = vi.fn();
  const ui = (d?: boolean) => (
    <EntryTable
      entries={entries}
      failedCount={0}
      entriesWithheld={false}
      onBrowse={onBrowse}
      onClear={onClear}
      disabled={d}
    />
  );
  const { rerender } = render(ui(disabled));
  const rows = () => screen.getAllByRole("row").slice(1);
  return { onBrowse, onClear, rows, setDisabled: (d: boolean) => rerender(ui(d)) };
}

test("exactly one row is in the tab order, and Up and Down move focus between rows", async () => {
  const user = userEvent.setup();
  const { rows } = setup();
  expect(rows().map((r) => r.getAttribute("tabindex"))).toEqual(["0", "-1", "-1"]);
  rows()[0].focus();
  await user.keyboard("{ArrowDown}");
  expect(rows()[1]).toHaveFocus();
  expect(rows().map((r) => r.getAttribute("tabindex"))).toEqual(["-1", "0", "-1"]);
  await user.keyboard("{ArrowDown}{ArrowDown}");
  expect(rows()[2]).toHaveFocus();
  await user.keyboard("{ArrowUp}");
  expect(rows()[1]).toHaveFocus();
});

test("Enter browses the focused row; Delete clears it only when assigned", async () => {
  const user = userEvent.setup();
  const { rows, onBrowse, onClear } = setup();
  rows()[1].focus();
  await user.keyboard("{Enter}");
  expect(onBrowse).toHaveBeenCalledWith("b");
  await user.keyboard("{Delete}");
  expect(onClear).not.toHaveBeenCalled();
  await user.keyboard("{ArrowDown}{Delete}");
  expect(onClear).toHaveBeenCalledWith("c");
});

test("Enter on a row's own button is that button's click, not a second browse", async () => {
  const user = userEvent.setup();
  const { onBrowse } = setup();
  await user.click(screen.getAllByRole("button", { name: /^Browse/ })[0]);
  onBrowse.mockClear();
  screen.getAllByRole("button", { name: /^Browse/ })[0].focus();
  await user.keyboard("{Enter}");
  expect(onBrowse).toHaveBeenCalledTimes(1);
});

test("row actions carry shortcut hints and keep their names", () => {
  setup();
  const browse = screen.getByRole("button", { name: "Browse… for /opt/a" });
  expect(browse).toHaveAttribute("aria-keyshortcuts", "Enter");
  expect(browse).toHaveAttribute("title", "Shortcut: Enter");
  const clear = screen.getByRole("button", { name: "Clear assigned file for /opt/a" });
  expect(clear).toHaveAttribute("aria-keyshortcuts", "Delete");
  expect(clear).toHaveAttribute("title", "Forget this file (nothing is deleted). Shortcut: Delete");
});

const outcome: DropOutcome = { matched: [["a", "C:\\x\\a.bin"]], unmatched: [], ambiguous: [] };
const fire = (type: DragDrop["type"], paths: string[] = []) => act(() => emit({ type, paths }));

async function mountWithDropResult() {
  const s = session(manifest({ entries }));
  vi.mocked(tp.session).mockResolvedValue(s);
  vi.mocked(tp.assignDropped).mockResolvedValue({ session: s, outcome });
  render(<TarpackView />);
  await screen.findByRole("heading", { level: 1 });
  await waitFor(() => expect(onDragDrop).toHaveBeenCalled());
  await act(async () => undefined);
  fire("enter", ["C:\\x\\a.bin"]);
  fire("drop", ["C:\\x\\a.bin"]);
  await screen.findByRole("region", { name: "Drop result" });
}

test("Escape with focus on the drop result's Dismiss moves focus to the tab-stop row", async () => {
  const user = userEvent.setup();
  await mountWithDropResult();
  // Move the tab stop off the first row to prove it is the tab stop that receives focus.
  act(() => screen.getAllByRole("row")[2].focus());
  act(() => screen.getByRole("button", { name: "Dismiss drop result" }).focus());
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("region", { name: "Drop result" })).toBeNull();
  expect(screen.getAllByRole("row")[2]).toHaveFocus();
});

test("Escape with focus elsewhere dismisses the result and leaves focus where it was", async () => {
  const user = userEvent.setup();
  await mountWithDropResult();
  const reload = screen.getByRole("button", { name: "Reload" });
  reload.focus();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("region", { name: "Drop result" })).toBeNull();
  expect(reload).toHaveFocus();
});

test("only the tab-stop row's own buttons are tabbable", () => {
  setup();
  const tabbable = (row: HTMLElement) =>
    Array.from(row.querySelectorAll("button")).map((b) => b.getAttribute("tabindex"));
  const rows = screen.getAllByRole("row").slice(1);
  expect(tabbable(rows[0])).toEqual(["0", "0"]);
  expect(tabbable(rows[1])).toEqual(["-1", "-1"]);
});

test("disabled: no row or row button is tabbable, row keys do nothing, and the tab stop comes back", async () => {
  const user = userEvent.setup();
  const { rows, onBrowse, onClear, setDisabled } = setup(true);
  for (const r of rows()) expect(r).toHaveAttribute("tabindex", "-1");
  for (const b of document.querySelectorAll("tbody button")) expect(b).toHaveAttribute("tabindex", "-1");
  act(() => rows()[0].focus());
  await user.keyboard("{Enter}{Delete}{ArrowDown}");
  expect(onBrowse).not.toHaveBeenCalled();
  expect(onClear).not.toHaveBeenCalled();
  expect(rows()[0]).toHaveFocus();
  setDisabled(false);
  expect(rows().map((r) => r.getAttribute("tabindex"))).toEqual(["0", "-1", "-1"]);
});

test("Clear leaves focus on the row: from its button by Delete and by click, and from the row itself", async () => {
  const user = userEvent.setup();
  const { rows, onClear } = setup();
  const clearBtn = () => screen.getAllByRole("button", { name: /^Clear/ })[0];
  // Clear disables itself in the real view; emulate that so a lost focus would show.
  onClear.mockImplementation(() => clearBtn().setAttribute("disabled", ""));
  act(() => clearBtn().focus());
  await user.keyboard("{Delete}");
  expect(onClear).toHaveBeenCalledTimes(1);
  expect(rows()[0]).toHaveFocus();

  clearBtn().removeAttribute("disabled");
  onClear.mockClear();
  act(() => clearBtn().focus());
  await user.click(clearBtn());
  expect(onClear).toHaveBeenCalledTimes(1);
  expect(rows()[0]).toHaveFocus();

  clearBtn().removeAttribute("disabled");
  onClear.mockClear();
  act(() => rows()[0].focus());
  await user.keyboard("{Delete}");
  expect(onClear).toHaveBeenCalledWith("a");
  expect(rows()[0]).toHaveFocus();
});
