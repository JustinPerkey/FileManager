import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { EntryStatus } from "./EntryStatus";

test.each([
  ["ready", "Ready"],
  ["missing", "Missing"],
  ["unassigned", "Not assigned"],
] as const)("%s shows %s", (status, label) => {
  render(<EntryStatus status={status} />);
  expect(screen.getByText(label)).toBeInTheDocument();
});
