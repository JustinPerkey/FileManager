import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, test, vi } from "vitest";
import { ConfirmDialog } from "./ConfirmDialog";

function Harness({ onConfirm = () => undefined }: { onConfirm?: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button onClick={() => setOpen(true)}>Open</button>
      <ConfirmDialog
        open={open}
        title="Replace a.tar?"
        body="It can't be undone."
        confirmLabel="Replace"
        onCancel={() => setOpen(false)}
        onConfirm={() => {
          onConfirm();
          setOpen(false);
        }}
      />
    </>
  );
}

test("focus starts on Cancel, Escape cancels, focus returns", async () => {
  const user = userEvent.setup();
  render(<Harness />);
  const opener = screen.getByRole("button", { name: "Open" });
  await user.click(opener);
  expect(screen.getByRole("heading", { name: "Replace a.tar?" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
  // jsdom does not turn Escape into the dialog's `cancel` event; browsers do.
  fireEvent(document.querySelector("dialog")!, new Event("cancel", { cancelable: true }));
  expect(document.querySelector("dialog")).not.toHaveAttribute("open");
  expect(opener).toHaveFocus();
});

test("Replace confirms", async () => {
  const onConfirm = vi.fn();
  const user = userEvent.setup();
  render(<Harness onConfirm={onConfirm} />);
  await user.click(screen.getByRole("button", { name: "Open" }));
  await user.click(screen.getByRole("button", { name: "Replace" }));
  expect(onConfirm).toHaveBeenCalledTimes(1);
});

test("no axe violations when open", async () => {
  const user = userEvent.setup();
  const { container } = render(<Harness />);
  await user.click(screen.getByRole("button", { name: "Open" }));
  expect((await axe(container)).violations).toEqual([]);
});
