import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import App from "./App";

test("renders the shell with the Tar Packager view", () => {
  render(<App />);
  expect(screen.getByRole("navigation", { name: "Tools" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Tar Packager" })).toBeInTheDocument();
});
