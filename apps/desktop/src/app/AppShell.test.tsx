import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, test, vi } from "vitest";
import { session } from "../tools/tarpack/fixtures";
import { AppShell } from "./AppShell";
import { tools as registry } from "../tools/registry";

vi.mock("../lib/tarpack", () => ({
  session: vi.fn(async () => session(null)),
  recentManifests: vi.fn(async () => []),
  onManifestChanged: vi.fn(async () => () => undefined),
}));
vi.mock("../lib/pipeline", () => ({
  session: vi.fn(async () => ({
    directory: null,
    steps: ["Verify", "RunBatch", "Schedule", "Tarpack"].map((id, i) => ({
      id,
      report: null,
      error: null,
      unlocked: i === 0,
    })),
    nextStep: "Verify",
    running: null,
    canRun: false,
  })),
}));

const tools = [
  { id: "a", label: "Alpha", view: () => <h1>Alpha view</h1> },
  { id: "b", label: "Beta", view: () => <h1>Beta view</h1> },
];

test("renders registry entries", () => {
  render(<AppShell tools={registry} />);
  expect(screen.getByRole("button", { name: "Pipeline" })).toHaveAttribute("aria-current", "page");
  expect(screen.getByRole("button", { name: "Tar Packager" })).toBeInTheDocument();
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
  await screen.findByRole("heading", { name: "Pipeline" });
  expect((await axe(container)).violations).toEqual([]);
});
