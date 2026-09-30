import { render, screen } from "@testing-library/react";
import { axe } from "vitest-axe";
import { expect, test } from "vitest";
import { DiagnosticList } from "./DiagnosticList";
import { diag } from "./fixtures";

test("renders nothing for an empty list", () => {
  const { container } = render(<DiagnosticList diagnostics={[]} label="X" />);
  expect(container).toBeEmptyDOMElement();
});

test("items have accessible position text and verbatim message", async () => {
  const { container } = render(
    <DiagnosticList diagnostics={[diag("file `a`: bad", 12, 5)]} label="Errors" />,
  );
  const item = screen.getByRole("list", { name: "Errors" }).querySelector("li")!;
  expect(item.textContent).toContain("Line 12, column 5: 12:5file `a`: bad");
  expect(item.querySelector('[aria-hidden="true"]')).toHaveTextContent("12:5");
  expect((await axe(container)).violations).toEqual([]);
});
