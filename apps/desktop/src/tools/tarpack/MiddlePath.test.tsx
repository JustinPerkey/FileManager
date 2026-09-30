import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { MiddlePath } from "./MiddlePath";

const path = "C:\\Users\\me\\build\\gateway.exe";

test("line is aria-hidden; full path is the accessible text and the title", () => {
  const { container } = render(<MiddlePath path={path} />);
  const root = container.firstElementChild!;
  expect(root).toHaveAttribute("title", path);
  expect(container.querySelector(".middle-path__line")).toHaveAttribute("aria-hidden", "true");
  expect(screen.getByText(path, { selector: ".middle-path__full" })).toBeInTheDocument();
});

test("head plus tail equal the path when the name is under the cap", () => {
  const { container } = render(<MiddlePath path={path} />);
  const head = container.querySelector(".middle-path__head")!.textContent;
  const tail = container.querySelector(".middle-path__tail")!.textContent;
  expect(head! + tail!).toBe(path);
});
