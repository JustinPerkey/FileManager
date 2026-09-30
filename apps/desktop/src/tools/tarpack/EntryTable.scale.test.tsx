import { render } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { SessionEntry } from "../../lib/generated/SessionEntry";
import { entry } from "./fixtures";

const renders = vi.hoisted(() => ({ n: 0 }));
vi.mock("./EntryStatus", () => ({
  EntryStatus: ({ status }: { status: string }) => {
    renders.n++;
    return <span>{status}</span>;
  },
}));

import { EntryTable } from "./EntryTable";

test("2,000 rows render; one status change re-renders one row", () => {
  const entries: SessionEntry[] = Array.from({ length: 2000 }, (_, i) => entry(`e${i}`));
  const onBrowse = vi.fn();
  const onClear = vi.fn();
  const props = { failedCount: 0, entriesWithheld: false, onBrowse, onClear };
  const { container, rerender } = render(<EntryTable entries={entries} {...props} />);
  expect(container.querySelectorAll("tbody tr")).toHaveLength(2000);
  renders.n = 0;
  const next = entries.map((x, i) => (i === 1000 ? { ...x, status: "ready" as const, assigned: "C:\\x" } : { ...x }));
  rerender(<EntryTable entries={next} {...props} />);
  expect(renders.n).toBe(1);
});
