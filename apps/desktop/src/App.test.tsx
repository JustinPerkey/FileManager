import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import App from "./App";

vi.mock("./lib/pipeline", () => ({
  session: vi.fn(async () => ({
    directory: null,
    steps: [],
    nextStep: null,
    running: null,
    canRun: false,
  })),
}));

test("renders the shell with the Pipeline view first", async () => {
  render(<App />);
  expect(screen.getByRole("navigation", { name: "Tools" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Pipeline" })).toHaveAttribute("aria-current", "page");
  expect(await screen.findByRole("heading", { name: "Pipeline" })).toBeInTheDocument();
});
