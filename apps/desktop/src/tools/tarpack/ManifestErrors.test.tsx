import { createRef } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, test, vi } from "vitest";
import { ManifestErrors } from "./ManifestErrors";
import { diag, failure, manifest } from "./fixtures";
import type { SessionManifest } from "../../lib/generated/SessionManifest";
import { DevOn } from "../../app/DevOn";

function setup(m: SessionManifest, expanded = true) {
  const onExpandedChange = vi.fn();
  const onEdit = vi.fn();
  const utils = render(
    <ManifestErrors
      manifest={m}
      expanded={expanded}
      onExpandedChange={onExpandedChange}
      onEdit={onEdit}
      reportRef={createRef()}
    />,
    { wrapper: DevOn },
  );
  return { ...utils, onExpandedChange, onEdit };
}

test("renders nothing with no errors or warnings", () => {
  const { container } = setup(manifest());
  expect(container).toBeEmptyDOMElement();
});

test("count heading, singular and plural", () => {
  const { unmount } = setup(manifest({ errors: [diag("a")] }));
  expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("1 error in this manifest");
  unmount();
  setup(manifest({ errors: [diag("a"), diag("b")] }));
  expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("2 errors in this manifest");
});

test("keyed on errorCount, not errors.length", () => {
  setup(manifest({ failedEntries: [failure(1)] }));
  expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("1 error in this manifest");
});

test("consequence lines", () => {
  const { unmount } = setup(manifest({ entriesWithheld: true, entries: [], errors: [diag("a")] }));
  expect(screen.getByText(/No files can be listed or built until the manifest errors are fixed\./)).toBeInTheDocument();
  unmount();
  const u2 = setup(manifest({ failedEntries: [failure(1)] }));
  expect(screen.getByText("1 file is left out of the list and the archive until it's fixed.")).toBeInTheDocument();
  u2.unmount();
  const u3 = setup(manifest({ failedEntries: [failure(1), failure(2)] }));
  expect(screen.getByText("2 files are left out of the list and the archive until they're fixed.")).toBeInTheDocument();
  u3.unmount();
  const { container } = setup(manifest({ errors: [diag("name bad")] }));
  expect(screen.getByText("Every file is listed and can still be built.")).toBeInTheDocument();
  expect(container.textContent).not.toMatch(/block|prevent/i);
});

test("report groups: whole manifest, then files with errors", () => {
  setup(manifest({ errors: [diag("name bad")], failedEntries: [failure(1), failure(2)] }));
  const h3s = screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent);
  expect(h3s).toEqual(["Whole manifest", "Files with errors"]);
  expect(screen.getAllByRole("heading", { level: 4 }).map((h) => h.textContent)).toEqual(["bad1", "bad2"]);
  expect(screen.getByRole("region", { name: /3 errors in this manifest/ })).toBeInTheDocument();
});

test("toggle has aria-expanded/controls and works by keyboard", async () => {
  const user = userEvent.setup();
  const { onExpandedChange, onEdit } = setup(manifest({ errors: [diag("a")] }), true);
  const toggle = screen.getByRole("button", { name: "Hide errors" });
  expect(toggle).toHaveAttribute("aria-expanded", "true");
  expect(toggle).toHaveAttribute("aria-controls", "manifest-error-report");
  toggle.focus();
  await user.keyboard("{Enter}");
  await user.keyboard(" ");
  expect(onExpandedChange).toHaveBeenCalledTimes(2);
  expect(onExpandedChange).toHaveBeenCalledWith(false);
  await user.click(screen.getByRole("button", { name: "Edit in editor" }));
  expect(onEdit).toHaveBeenCalled();
});

test("collapsed body stays mounted but hidden", async () => {
  const { container } = setup(manifest({ errors: [diag("a")] }), false);
  expect(container.querySelector("#manifest-error-report")).toHaveAttribute("hidden");
  expect(screen.getByRole("button", { name: "Show errors" })).toHaveAttribute("aria-expanded", "false");
  expect((await axe(container)).violations).toEqual([]);
});

test("warnings render collapsed alongside errors, verbatim", async () => {
  const msg = "file `long`: stored path /opt/x is 112 bytes, 100 bytes or longer";
  const { container } = setup(
    manifest({ errors: [diag("a")], warnings: [{ ...diag(msg), severity: "warning" }] }),
  );
  const details = container.querySelector("details")!;
  expect(details).not.toHaveAttribute("open");
  expect(details).toHaveTextContent("1 warning");
  expect(details).toHaveTextContent(msg);
  expect((await axe(container)).violations).toEqual([]);
});

test("warnings only", () => {
  setup(manifest({ warnings: [diag("w1"), diag("w2")] }));
  expect(screen.getByText("warnings")).toBeInTheDocument();
  expect(screen.queryByRole("heading", { level: 2 })).toBeNull();
});
