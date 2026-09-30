import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, test, vi } from "vitest";
import { EntryTable, type EntryTableProps } from "./EntryTable";
import { entry } from "./fixtures";
import type { SessionEntry } from "../../lib/generated/SessionEntry";

const e = (id: string, over: Partial<SessionEntry> = {}): SessionEntry => ({ ...entry(id), ...over });
const ready = (id: string, over: Partial<SessionEntry> = {}) =>
  e(id, { status: "ready", assigned: `C:\\src\\${id}.bin`, ...over });

function setup(over: Partial<EntryTableProps> = {}) {
  const props: EntryTableProps = {
    entries: [ready("a"), e("b", { status: "missing", assigned: "C:\\gone\\b.bin" }), e("c")],
    failedCount: 0,
    entriesWithheld: false,
    onBrowse: vi.fn(),
    onClear: vi.fn(),
    ...over,
  };
  return { props, ...render(<EntryTable {...props} />) };
}

const summary = () => document.querySelector(".entry-summary")!;

test("renders a captioned table with column headers and one row per entry", () => {
  setup();
  expect(screen.getByRole("table", { name: "Files in this package" })).toBeInTheDocument();
  expect(screen.getAllByRole("columnheader")).toHaveLength(7);
  expect(screen.getAllByRole("row")).toHaveLength(4);
  expect(screen.getByText("Ready")).toBeInTheDocument();
  expect(screen.getByText("Missing")).toBeInTheDocument();
  expect(screen.getByText("Not assigned")).toBeInTheDocument();
  expect(screen.getByText("(not found)")).toBeInTheDocument();
  expect(screen.getByText("/opt/a")).toBeInTheDocument();
  expect(screen.getAllByText("rwxr-xr-x")).toHaveLength(3);
});

test("summary: mixed", () => {
  setup();
  expect(summary()).toHaveTextContent("1 of 3 files ready");
});

test("summary: all ready, with and without failed entries", () => {
  const { rerender, props } = setup({ entries: [ready("a"), ready("b")] });
  expect(summary()).toHaveTextContent("All 2 files ready");
  rerender(<EntryTable {...props} entries={[ready("a"), ready("b")]} failedCount={1} />);
  expect(summary().textContent).not.toMatch(/^All/);
  expect(summary()).toHaveTextContent("2 of 2 files ready · 1 file left out (errors)");
  rerender(<EntryTable {...props} entries={[ready("a")]} failedCount={3} />);
  expect(summary()).toHaveTextContent("1 of 1 file ready · 3 files left out (errors)");
});

test("line-ending clause and marker only for normalizeEol rows", () => {
  const { rerender, props } = setup({ entries: [ready("a")] });
  expect(summary()).not.toHaveTextContent("line endings");
  expect(screen.queryByText("CRLF → LF")).toBeNull();
  rerender(<EntryTable {...props} entries={[ready("a", { normalizeEol: true }), ready("b")]} />);
  expect(summary()).toHaveTextContent("1 file converts line endings to LF");
  rerender(
    <EntryTable {...props} entries={[ready("a", { normalizeEol: true }), ready("b", { normalizeEol: true })]} />,
  );
  expect(summary()).toHaveTextContent("2 files convert line endings to LF");
  const row = screen.getAllByRole("row")[1];
  expect(within(row).getByText("CRLF → LF")).toBeInTheDocument();
  expect(within(row).getByText("line endings converted to LF")).toBeInTheDocument();
  expect(row.querySelector(".eol-marker")).not.toHaveAttribute("tabindex");
});

test("empty cases render no table", () => {
  const { rerender, props } = setup({ entries: [], entriesWithheld: true });
  expect(screen.queryByRole("table")).toBeNull();
  expect(screen.getByText(/A manifest error hides them until it’s fixed\./)).toBeInTheDocument();
  expect(screen.getByText(/Fix them in your editor, then Reload\./)).toBeInTheDocument();
  rerender(<EntryTable {...props} entries={[]} entriesWithheld={false} failedCount={2} />);
  expect(screen.getByText(/Every file in this manifest has errors\./)).toBeInTheDocument();
  expect(screen.getByText(/Fix them in your editor, then Reload\./)).toBeInTheDocument();
  rerender(<EntryTable {...props} entries={[]} entriesWithheld={false} failedCount={0} />);
  expect(screen.getByText("This manifest lists no files.")).toBeInTheDocument();
  expect(screen.getByText("[[file]]")).toHaveClass("mono");
  expect(document.querySelector(".entry-summary")).toBeNull();
});

test("Clear is disabled when unassigned; buttons call props with the id", async () => {
  const { props } = setup();
  const rows = screen.getAllByRole("row");
  expect(within(rows[3]).getByRole("button", { name: /^Clear/ })).toBeDisabled();
  await userEvent.click(within(rows[1]).getByRole("button", { name: /^Browse/ }));
  await userEvent.click(within(rows[1]).getByRole("button", { name: /^Clear/ }));
  expect(props.onBrowse).toHaveBeenCalledWith("a");
  expect(props.onClear).toHaveBeenCalledWith("a");
});

test("action names use the target, so repeated sources stay distinct; Clear has a title", () => {
  setup({
    entries: [
      ready("a", { source: "run.sh", targetPath: "/opt/one/run.sh" }),
      ready("b", { source: "run.sh", targetPath: "/opt/two/run.sh" }),
    ],
  });
  expect(screen.getByRole("button", { name: "Browse… for /opt/one/run.sh" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Browse… for /opt/two/run.sh" })).toBeInTheDocument();
  const clear = screen.getByRole("button", { name: "Clear assigned file for /opt/one/run.sh" });
  expect(clear).toHaveAttribute("title", "Forget this file (nothing is deleted). Shortcut: Delete");
  expect(clear).toHaveAccessibleDescription("Forget this file (nothing is deleted). Shortcut: Delete");
  expect(screen.getByRole("button", { name: "Clear assigned file for /opt/two/run.sh" })).toBeInTheDocument();
});

test("seven classed columns; cell labels are aria-hidden", () => {
  const { container } = setup();
  expect(container.querySelectorAll("colgroup > col")).toHaveLength(7);
  expect(container.querySelector(".entry-table__col--windows")).not.toBeNull();
  const labels = container.querySelectorAll(".entry-table__cell-label");
  expect(labels.length).toBe(12);
  labels.forEach((l) => expect(l).toHaveAttribute("aria-hidden", "true"));
});

test("target path span is mono, exact, with a wbr after each slash; no td is mono", () => {
  const target = "/opt/gateway/bin/gateway";
  const { container } = setup({ entries: [ready("a", { targetPath: target })] });
  const span = container.querySelector(".entry-table__target-path")!;
  expect(span.textContent).toBe(target);
  expect(span).toHaveClass("mono");
  expect(span.querySelectorAll("wbr")).toHaveLength(4);
  expect(span.previousElementSibling).toHaveClass("entry-table__cell-label");
  container.querySelectorAll("td").forEach((td) => expect(td).not.toHaveClass("mono"));
});

test("Windows location: full path is the accessible text once, including for a long path", () => {
  const long = `C:\\${"d".repeat(190)}\\file.txt`;
  const { container } = setup({ entries: [ready("a", { assigned: long })] });
  expect(container.querySelector(".middle-path")).toHaveAttribute("title", long);
  expect(screen.getByText(long, { selector: ".middle-path__full" })).toBeInTheDocument();
});

test("has no axe violations", async () => {
  const { container } = setup({
    entries: [ready("a", { normalizeEol: true }), e("b", { status: "missing", assigned: "C:\\gone\\b.bin" }), e("c")],
  });
  expect((await axe(container)).violations).toEqual([]);
});
