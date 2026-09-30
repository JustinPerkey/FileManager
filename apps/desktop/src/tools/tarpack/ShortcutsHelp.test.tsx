import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, test, vi } from "vitest";
import { SHORTCUTS, ShortcutsHelp } from "./ShortcutsHelp";

const button = () => screen.getByRole("button", { name: "Keyboard shortcuts" });

test("opens from the button by keyboard and lists every shortcut, including F8", async () => {
  const user = userEvent.setup();
  const { container } = render(<ShortcutsHelp />);
  const pop = container.querySelector("[popover]") as HTMLElement;
  expect(pop).not.toBeVisible();
  await user.tab();
  expect(button()).toHaveFocus();
  await user.keyboard("{Enter}");
  expect(pop).toBeVisible();
  const table = screen.getByRole("table", { name: "Keyboard shortcuts" });
  for (const s of SHORTCUTS) expect(table).toHaveTextContent(s.action);
  expect(table).toHaveTextContent("F8");
  expect(table).toHaveTextContent("Show errors");
  expect(pop.querySelectorAll("kbd").length).toBeGreaterThanOrEqual(SHORTCUTS.length);
  expect((await axe(container)).violations).toEqual([]);
});

test("Escape closes it and returns focus to the button, without reaching the view", async () => {
  const user = userEvent.setup();
  const outer = vi.fn();
  window.addEventListener("keydown", outer);
  const { container } = render(<ShortcutsHelp />);
  await user.tab();
  await user.keyboard("{Enter}");
  await user.keyboard("{Escape}");
  expect(container.querySelector("[popover]")).not.toBeVisible();
  expect(button()).toHaveFocus();
  expect(outer.mock.calls.filter(([e]) => (e as KeyboardEvent).key === "Escape")).toHaveLength(0);
  window.removeEventListener("keydown", outer);
});

test("reports its open state, including false on unmount", async () => {
  const user = userEvent.setup();
  const onOpenChange = vi.fn();
  const { unmount } = render(<ShortcutsHelp onOpenChange={onOpenChange} />);
  await user.tab();
  await user.keyboard("{Enter}");
  expect(onOpenChange).toHaveBeenLastCalledWith(true);
  await user.keyboard("{Escape}");
  expect(onOpenChange).toHaveBeenLastCalledWith(false);
  onOpenChange.mockClear();
  await user.keyboard("{Enter}");
  expect(onOpenChange).toHaveBeenLastCalledWith(true);
  unmount();
  expect(onOpenChange).toHaveBeenLastCalledWith(false);
});
