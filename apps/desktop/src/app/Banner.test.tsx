import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, test, vi } from "vitest";
import { Banner } from "./Banner";

test.each([
  ["info", "Information:"],
  ["warn", "Warning:"],
  ["error", "Error:"],
] as const)("%s tone has the right live role and hidden word", async (tone, word) => {
  const { container } = render(<Banner tone={tone} message="Something" />);
  const el = container.querySelector(".banner")!;
  if (tone === "error") expect(el).toHaveAttribute("role", "alert");
  else {
    expect(el).not.toHaveAttribute("role");
    expect(el).not.toHaveAttribute("aria-live");
  }
  expect(el).toHaveClass(`banner--${tone}`);
  expect(el).toHaveTextContent(`${word} Something`);
  expect(screen.queryByRole("button")).toBeNull();
  expect((await axe(container)).violations).toEqual([]);
});

test("action and dismiss", async () => {
  const user = userEvent.setup();
  const onAction = vi.fn();
  const onDismiss = vi.fn();
  render(<Banner tone="warn" message="m" action={{ label: "Reload", onAction }} onDismiss={onDismiss} />);
  await user.click(screen.getByRole("button", { name: "Reload" }));
  await user.click(screen.getByRole("button", { name: "Dismiss" }));
  expect(onAction).toHaveBeenCalled();
  expect(onDismiss).toHaveBeenCalled();
});

test("renders children", () => {
  render(
    <Banner tone="error" message="m">
      <details>
        <summary>Details</summary>
      </details>
    </Banner>,
  );
  expect(screen.getByText("Details")).toBeInTheDocument();
});
