import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, test } from "vitest";
import { AppShell } from "./AppShell";
import { tools as registry } from "../tools/registry";

const tools = [
  { id: "a", label: "Alpha", view: () => <h1>Alpha view</h1> },
  { id: "b", label: "Beta", view: () => <h1>Beta view</h1> },
];

test("renders registry entries", () => {
  render(<AppShell tools={registry} />);
  expect(screen.getByRole("button", { name: "Tar Packager" })).toHaveAttribute("aria-current", "page");
});

test("selecting a tool renders its view", async () => {
  const user = userEvent.setup();
  render(<AppShell tools={tools} />);
  expect(screen.getByRole("heading", { name: "Alpha view" })).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Beta" }));
  expect(screen.getByRole("heading", { name: "Beta view" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Beta" })).toHaveAttribute("aria-current", "page");
});

test("shows an empty state with no tools", () => {
  render(<AppShell tools={[]} />);
  expect(screen.getByText("No tools available.")).toBeInTheDocument();
});

test("has no axe violations", async () => {
  const { container } = render(<AppShell tools={registry} />);
  expect((await axe(container)).violations).toEqual([]);
});
