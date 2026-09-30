import { render, screen, within } from "@testing-library/react";
import { axe } from "vitest-axe";
import { expect, test } from "vitest";
import { BuildReport } from "./BuildReport";
import { diag, failure } from "./fixtures";

test("renders nothing when everything is empty", () => {
  const { container } = render(<BuildReport leftOut={[]} manifestErrors={[]} warnings={[]} />);
  expect(container).toBeEmptyDOMElement();
});

test("each section is present only when non-empty; entry without id; every error", async () => {
  const { container } = render(
    <BuildReport
      leftOut={[failure(1, { id: null, source: null, errors: [diag("one", 2, 1), diag("two", 3, 4)] })]}
      manifestErrors={[diag("bad name")]}
      warnings={[{ ...diag("odd"), severity: "warning" }]}
    />,
  );
  const region = screen.getByRole("region", { name: "Build report" });
  expect(region).toHaveAttribute("tabindex", "0");
  expect(screen.getByRole("heading", { level: 3, name: "Left out of the archive (1)" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { level: 3, name: "Manifest errors (1)" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { level: 4, name: "Entry #1" })).toBeInTheDocument();
  expect(within(region).getByText("one")).toBeInTheDocument();
  expect(within(region).getByText("two")).toBeInTheDocument();
  expect(container.querySelector("details")).not.toHaveAttribute("open");
  expect((await axe(container)).violations).toEqual([]);
});

test("warnings only", () => {
  render(
    <BuildReport leftOut={[]} manifestErrors={[]} warnings={[{ ...diag("odd"), severity: "warning" }]} />,
  );
  expect(screen.queryByRole("heading", { name: /Left out/ })).toBeNull();
  expect(screen.queryByRole("heading", { name: /Manifest errors/ })).toBeNull();
  expect(screen.getByRole("heading", { name: "Warnings (1)" })).toBeInTheDocument();
});
