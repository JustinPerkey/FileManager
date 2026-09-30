import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import App from "./App";
import { session } from "./tools/tarpack/fixtures";

vi.mock("./lib/tarpack", () => ({
  session: vi.fn(async () => session(null)),
  recentManifests: vi.fn(async () => []),
  onManifestChanged: vi.fn(async () => () => undefined),
}));

test("renders the shell with the Tar Packager view", async () => {
  render(<App />);
  expect(screen.getByRole("navigation", { name: "Tools" })).toBeInTheDocument();
  expect(await screen.findByRole("heading", { name: "Tar Packager" })).toBeInTheDocument();
});
