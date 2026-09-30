import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, test, vi } from "vitest";
import type { DropOutcome } from "../../lib/generated/DropOutcome";
import type { UnmatchedReason } from "../../lib/generated/UnmatchedReason";
import { DropResult, dropResultText } from "./DropResult";

const empty: DropOutcome = { matched: [], unmatched: [], ambiguous: [] };
const out = (over: Partial<DropOutcome>): DropOutcome => ({
  ...empty,
  ...over,
});
const show = (o: DropOutcome, failed = false, onDismiss = vi.fn()) =>
  render(<DropResult outcome={o} hasFailedEntries={failed} onDismiss={onDismiss} />);
const lines = (c: HTMLElement) => Array.from(c.querySelectorAll("li")).map((l) => l.textContent);

test("matched line", () => {
  const { container } = show(
    out({
      matched: [
        ["a", "C:\\x\\a.dll"],
        ["b", "C:\\x\\b.dll"],
        ["c", "C:\\x\\c"],
      ],
    }),
  );
  expect(lines(container)).toEqual(["3 matched"]);
  expect(container.querySelector(".drop-result__icon--ok svg")).not.toBeNull();
});

const cases: [UnmatchedReason, string, "muted" | "warn", boolean?][] = [
  ["noEntry", "1 not in the manifest: notes.txt", "muted"],
  ["alreadyAssigned", "1 already assigned, left unchanged: notes.txt", "muted"],
  ["folderNoMatch", "1 folder with nothing to match: notes.txt", "muted"],
  ["linkNotFollowed", "1 link not followed: notes.txt", "muted"],
  ["notFound", "1 no longer found: notes.txt", "warn"],
  ["unreadable", "1 could not be read: notes.txt", "warn"],
  ["notUnicode", "1 can't be assigned: the path has unsupported characters. Rename it: notes.txt", "warn"],
];
test.each(cases)("unmatched reason %s has its copy and icon", (reason, text, tone) => {
  const { container } = show(out({ unmatched: [{ path: "C:\\d\\notes.txt", reason }] }));
  expect(lines(container)).toEqual([text]);
  expect(container.querySelector(`.drop-result__icon--${tone} svg`)).not.toBeNull();
  expect(screen.getByText("notes.txt")).toHaveAttribute("title", "C:\\d\\notes.txt");
  expect(container.textContent).not.toContain(reason === "noEntry" ? "reason" : `${reason}`);
});

test("plural phrases", () => {
  const u = (reason: UnmatchedReason) => [
    { path: "C:\\a", reason },
    { path: "C:\\b", reason },
  ];
  const { container } = show(out({ unmatched: [...u("folderNoMatch"), ...u("linkNotFollowed")] }));
  expect(lines(container)).toEqual(["2 folders with nothing to match: a, b", "2 links not followed: a, b"]);
});

test("two items with one reason share one line", () => {
  const { container } = show(
    out({
      unmatched: [
        { path: "C:\\x\\app.dll", reason: "alreadyAssigned" },
        { path: "C:\\x\\core.dll", reason: "alreadyAssigned" },
      ],
    }),
  );
  expect(lines(container)).toEqual(["2 already assigned, left unchanged: app.dll, core.dll"]);
});

test("ambiguous line", () => {
  const { container } = show(
    out({
      ambiguous: [{ id: "app", candidates: ["C:\\a\\app.dll", "C:\\b\\app.dll"] }],
    }),
  );
  expect(lines(container)).toEqual(["1 ambiguous: app.dll could be 2 files — use Browse"]);
});

test("U+FFFD paths render", () => {
  const { container } = show(
    out({
      unmatched: [{ path: "C:\\d\\bad\uFFFDname.txt", reason: "noEntry" }],
    }),
  );
  expect(lines(container)).toEqual(["1 not in the manifest: bad\uFFFDname.txt"]);
});

test("very long lists are capped", () => {
  const unmatched = Array.from({ length: 20 }, (_, i) => ({
    path: `C:\\f${i}`,
    reason: "noEntry" as const,
  }));
  const { container } = show(out({ unmatched }));
  expect(lines(container)[0]).toMatch(/^20 not in the manifest: f0, .*f7 and 12 more$/);
});

test("failed entries: noEntry reads not matched and the hint follows; also after folderNoMatch alone", () => {
  const a = show(out({ unmatched: [{ path: "C:\\notes.txt", reason: "noEntry" }] }), true);
  expect(lines(a.container)).toEqual([
    "1 not matched: notes.txt",
    "Files for entries with errors can't be matched until those errors are fixed.",
  ]);
  a.unmount();
  const b = show(out({ unmatched: [{ path: "C:\\dir", reason: "folderNoMatch" }] }), true);
  expect(lines(b.container)).toHaveLength(2);
  b.unmount();
  const c = show(out({ unmatched: [{ path: "C:\\x", reason: "notFound" }] }), true);
  expect(lines(c.container)).toEqual(["1 no longer found: x"]);
  c.unmount();
  const d = show(out({ unmatched: [{ path: "C:\\notes.txt", reason: "noEntry" }] }), false);
  expect(lines(d.container)).toEqual(["1 not in the manifest: notes.txt"]);
});

const combined = out({
  matched: [["a", "C:\\a"]],
  unmatched: [
    { path: "C:\\x\\notes.txt", reason: "noEntry" },
    { path: "C:\\x\\gone.txt", reason: "notFound" },
    { path: "C:\\x\\core.dll", reason: "alreadyAssigned" },
  ],
  ambiguous: [{ id: "app", candidates: ["C:\\a\\app.dll", "C:\\b\\app.dll"] }],
});

test("combined outcome: order, and dropResultText matches the rendered lines", () => {
  const { container } = show(combined, true);
  const rendered = lines(container);
  expect(rendered.map((l) => l!.split(":")[0])).toEqual([
    "1 matched",
    "1 not matched",
    "1 already assigned, left unchanged",
    "1 no longer found",
    "Files for entries with errors can't be matched until those errors are fixed.",
    "1 ambiguous",
  ]);
  expect(dropResultText(combined, true)).toBe(rendered.map((l) => `${l}.`.replace(/\.\.$/, ".")).join(" "));
  expect(dropResultText(out({ matched: [["a", "b"]] }), false)).toBe("1 matched.");
});

test("dismiss by keyboard; no live role; axe clean", async () => {
  const user = userEvent.setup();
  const onDismiss = vi.fn();
  const { container } = show(combined, true, onDismiss);
  expect(container.querySelector("[role=status],[role=alert],[aria-live]")).toBeNull();
  expect((await axe(container)).violations).toEqual([]);
  await user.tab();
  expect(screen.getByRole("button", { name: "Dismiss drop result" })).toHaveFocus();
  await user.keyboard("{Enter}");
  expect(onDismiss).toHaveBeenCalled();
});

test("an outcome with nothing in it says so", () => {
  const { container } = show(empty);
  expect(lines(container)).toEqual(["Nothing was dropped that could be matched."]);
  expect(dropResultText(empty, false)).toBe("Nothing was dropped that could be matched.");
});
