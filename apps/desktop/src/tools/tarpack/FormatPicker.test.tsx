import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { FORMATS } from "./fixtures";
import { FormatPicker } from "./FormatPicker";

test("options in the given order with session extensions; selected value", () => {
  render(<FormatPicker formats={[...FORMATS]} value="tarZst" disabled={false} onChange={() => undefined} />);
  const select = screen.getByRole("combobox", { name: "Format" });
  expect(Array.from(select.querySelectorAll("option")).map((o) => o.textContent)).toEqual([
    "tar (.tar)",
    "gzip (.tar.gz)",
    "zstd (.tar.zst)",
    "xz (.tar.xz)",
  ]);
  expect(select).toHaveValue("tarZst");
});

test("keyboard selection calls onChange with the format", async () => {
  const onChange = vi.fn();
  render(<FormatPicker formats={[...FORMATS]} value="tar" disabled={false} onChange={onChange} />);
  await userEvent.selectOptions(screen.getByRole("combobox", { name: "Format" }), "tarXz");
  expect(onChange).toHaveBeenCalledWith("tarXz");
});

test("disabled", () => {
  render(<FormatPicker formats={[...FORMATS]} value="tar" disabled onChange={() => undefined} />);
  expect(screen.getByRole("combobox", { name: "Format" })).toBeDisabled();
});
