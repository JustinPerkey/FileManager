import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, test, vi } from "vitest";
import type { DropOutcome } from "../../lib/generated/DropOutcome";
import type { UnmatchedReason } from "../../lib/generated/UnmatchedReason";
import { DropResult, dropResultText } from "./DropResult";
import { entry } from "./fixtures";

const empty: DropOutcome = { matched: [], unmatched: [], ambiguous: [] };
const out = (over: Partial<DropOutcome>): DropOutcome => ({
  ...empty,
  ...over,
});
const entries = [entry("app"), entry("core"), entry("other")];
const show = (o: DropOutcome, failed = false, onDismiss = vi.fn()) =>
  render(<DropResult outcome={o} entries={entries} hasFailedEntries={failed} onDismiss={onDismiss} />);
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
  [
    "notUnicode",
    "1 not assigned, unsupported characters in its path \u2014 rename it or its folder: notes.txt",
    "warn",
  ],
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

test("ambiguous: an entry with two candidates", () => {
  const { container } = show(
    out({ ambiguous: [{ id: "app", candidates: ["C:\\a\\app.dll", "C:\\b\\app.dll"] }] }),
  );
  expect(lines(container)).toEqual([
    "1 ambiguous, not assigned \u2014 use Browse\u2026 on its row: /opt/app (2 files)",
  ]);
  expect(container.querySelector(".mono")).toHaveTextContent("/opt/app");
  expect(screen.getByText("/opt/app").closest("[title]")).toHaveAttribute(
    "title",
    "Matching files:\nC:\\a\\app.dll\nC:\\b\\app.dll",
  );
});

test("ambiguous: two entries sharing one candidate", () => {
  const c = ["C:\\x\\app.dll"];
  const o = out({
    ambiguous: [
      { id: "app", candidates: c },
      { id: "core", candidates: c },
    ],
  });
  const { container } = show(o);
  expect(lines(container)).toEqual([
    "2 ambiguous, not assigned \u2014 use Browse\u2026 on their rows: /opt/app (app.dll fits more than one entry), /opt/core (app.dll fits more than one entry)",
  ]);
});

test("ambiguous: an id not in entries falls back to the file name, never the id", () => {
  const { container } = show(out({ ambiguous: [{ id: "ghost", candidates: ["C:\\x\\app.dll"] }] }));
  expect(lines(container)[0]).toContain("on its row: app.dll (app.dll fits more than one entry)");
  expect(container.textContent).not.toContain("ghost");
});

test("notUnicode plural copy", () => {
  const { container } = show(
    out({
      unmatched: [
        { path: "C:\\a\\x.pdf", reason: "notUnicode" },
        { path: "C:\\a\\y.pdf", reason: "notUnicode" },
      ],
    }),
  );
  expect(lines(container)).toEqual([
    "2 not assigned, unsupported characters in their paths \u2014 rename them or their folders: x.pdf, y.pdf",
  ]);
});

test("U+FFFD paths render", () => {
  const { container } = show(
    out({
      unmatched: [{ path: "C:\\d\\bad\uFFFDname.txt", reason: "noEntry" }],
    }),
  );
  expect(lines(container)).toEqual(["1 not in the manifest: bad\uFFFDname.txt"]);
});

test("more than 8 names: 8 shown, full count, and the same in dropResultText", () => {
  const unmatched = Array.from({ length: 9 }, (_, i) => ({ path: `C:\\f${i}`, reason: "noEntry" as const }));
  const o = out({ unmatched });
  const { container } = show(o);
  expect(lines(container)[0]).toBe("9 not in the manifest: f0, f1, f2, f3, f4, f5, f6, f7, and 1 more");
  expect(screen.queryByText("f8")).toBeNull();
  expect(dropResultText(o, entries, false)).toBe(
    "9 not in the manifest: f0, f1, f2, f3, f4, f5, f6, f7, and 1 more.",
  );
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
    "1 ambiguous, not assigned \u2014 use Browse\u2026 on its row",
  ]);
  expect(dropResultText(combined, entries, true)).toBe(
    rendered.map((l) => (l!.endsWith(".") ? l : `${l}.`)).join(" "),
  );
  expect(dropResultText(out({ matched: [["a", "b"]] }), entries, false)).toBe("1 matched.");
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
  expect(lines(container)).toEqual(["Nothing was matched"]);
  expect(dropResultText(empty, entries, false)).toBe("Nothing was matched.");
});
