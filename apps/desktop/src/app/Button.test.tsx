import { createRef } from "react";
import { render, screen } from "@testing-library/react";
import { axe } from "vitest-axe";
import { expect, test } from "vitest";
import { Button } from "./Button";

test("variants", () => {
  render(
    <>
      <Button variant="primary">P</Button>
      <Button>S</Button>
      <Button variant="quiet">Q</Button>
    </>,
  );
  expect(screen.getByText("P")).toHaveClass("btn", "btn--primary");
  expect(screen.getByText("S")).toHaveClass("btn");
  expect(screen.getByText("S")).not.toHaveClass("btn--primary", "btn--quiet");
  expect(screen.getByText("Q")).toHaveClass("btn--quiet");
  expect(screen.getByText("S")).toHaveAttribute("type", "button");
});

test("forwards ref and disabled", () => {
  const ref = createRef<HTMLButtonElement>();
  render(<Button ref={ref} disabled>Go</Button>);
  expect(ref.current).toBe(screen.getByRole("button"));
  expect(ref.current).toBeDisabled();
});

test("busy sets aria-busy and disables", () => {
  render(<Button busy>Go</Button>);
  const b = screen.getByRole("button", { name: "Go" });
  expect(b).toHaveAttribute("aria-busy", "true");
  expect(b).toBeDisabled();
});

test("icon renders aria-hidden and no axe violations", async () => {
  const { container } = render(<Button icon="folder">Open</Button>);
  expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  expect((await axe(container)).violations).toEqual([]);
});
