import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { ToolNav } from "./ToolNav";

const View = () => null;
const tools = [
  { id: "a", label: "Alpha", view: View },
  { id: "b", label: "Beta", view: View },
  { id: "c", label: "Gamma", view: View },
];

test("marks the active item with aria-current", () => {
  render(<ToolNav tools={tools} activeId="b" onSelect={() => {}} />);
  expect(screen.getByRole("button", { name: "Beta" })).toHaveAttribute("aria-current", "page");
  expect(screen.getByRole("button", { name: "Alpha" })).not.toHaveAttribute("aria-current");
});

test("arrow keys move focus and wrap", async () => {
  const user = userEvent.setup();
  render(<ToolNav tools={tools} activeId="a" onSelect={() => {}} />);
  await user.tab();
  expect(screen.getByRole("button", { name: "Alpha" })).toHaveFocus();
  await user.keyboard("{ArrowDown}");
  expect(screen.getByRole("button", { name: "Beta" })).toHaveFocus();
  await user.keyboard("{ArrowUp}{ArrowUp}");
  expect(screen.getByRole("button", { name: "Gamma" })).toHaveFocus();
});

test("Enter and Space select the focused item", async () => {
  const user = userEvent.setup();
  const onSelect = vi.fn();
  render(<ToolNav tools={tools} activeId="a" onSelect={onSelect} />);
  await user.tab();
  await user.keyboard("{ArrowDown}{Enter}");
  expect(onSelect).toHaveBeenLastCalledWith("b");
  await user.keyboard("{ArrowDown} ");
  expect(onSelect).toHaveBeenLastCalledWith("c");
});
