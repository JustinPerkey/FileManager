import { render, screen, within } from "@testing-library/react";
import { axe } from "vitest-axe";
import { expect, test } from "vitest";
import { FailureList } from "./FailureList";
import { diag, failure } from "./fixtures";

test("renders nothing for an empty list", () => {
  const { container } = render(<FailureList failures={[]} headingLevel={4} />);
  expect(container).toBeEmptyDOMElement();
});

test("names by id, by index, and shows source and line", async () => {
  const { container } = render(
    <FailureList
      headingLevel={3}
      failures={[
        failure(1),
        failure(2, { id: null, source: "b.bin" }),
        failure(3, { id: null, source: null }),
      ]}
    />,
  );
  expect(screen.getByRole("heading", { level: 3, name: "bad1" })).toBeInTheDocument();
  const second = screen.getByRole("heading", { name: "Entry #2" }).closest("li")!;
  expect(second).toHaveTextContent("source b.bin · [[file]] on line 10");
  const third = screen.getByRole("heading", { name: "Entry #3" }).closest("li")!;
  expect(third).not.toHaveTextContent("source");
  expect(third).toHaveTextContent("[[file]] on line 15");
  expect((await axe(container)).violations).toEqual([]);
});

test("lists every error of a multi-error entry", () => {
  render(
    <FailureList
      headingLevel={4}
      failures={[failure(1, { errors: [diag("one", 2, 1), diag("two", 3, 4), diag("three", 4, 2)] })]}
    />,
  );
  const list = screen.getByRole("list", { name: "Errors in bad1" });
  expect(within(list).getAllByRole("listitem")).toHaveLength(3);
  expect(list).toHaveTextContent("Line 3, column 4:");
});
