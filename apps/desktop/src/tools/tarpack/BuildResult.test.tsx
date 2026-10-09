import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { beforeEach, expect, test, vi } from "vitest";
import { BuildResult } from "./BuildResult";
import { diag, entry, failure, summary } from "./fixtures";
import { DevOn } from "../../app/DevOn";

// user-event installs its own clipboard on setup(), so the spy goes on after it.
function userWithClipboard() {
  const user = userEvent.setup();
  const spy = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue(undefined);
  return Object.assign(user, { writeText: spy });
}
beforeEach(() => vi.restoreAllMocks());

const show = (s = summary(), onReveal = () => undefined) =>
  render(
    <BuildResult
      result={{ ok: s }}
      entries={[entry("gateway", "gateway.conf")]}
      onReveal={onReveal}
      onDismiss={() => undefined}
    />,
    { wrapper: DevOn },
  );

test("shows every field and copies exact text", async () => {
  const user = userWithClipboard();
  const writeText = user.writeText;
  const s = summary({
    normalizedEntries: [
      { id: "gateway", crlfReplaced: 3 },
      { id: "core", crlfReplaced: 0 },
    ],
  });
  const { container } = show(s);
  expect(screen.getByRole("heading", { level: 2, name: "Created gateway.tar.zst" })).toBeInTheDocument();
  expect(screen.getByText(s.path)).toBeInTheDocument();
  expect(container.querySelector(".build-result__facts")?.textContent).toBe(
    "zstd · 2 files, 1 folder · 2.0 KB (10.0 MB uncompressed)",
  );
  expect(screen.getByText(s.sha256Hex)).toBeInTheDocument();
  expect(screen.getByText(s.extractCommand)).toBeInTheDocument();
  expect(
    screen.getByText("Run this in the folder that holds the file. -P keeps the absolute paths."),
  ).toBeInTheDocument();
  expect(screen.getByText(/gateway\.conf/, { selector: "li span" })).toBeInTheDocument();
  expect(screen.getByText("3")).toBeInTheDocument();
  expect(screen.getByText("no CRLF found", { exact: false })).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Copy SHA-256" }));
  expect(writeText).toHaveBeenLastCalledWith(s.sha256Hex);
  expect(await screen.findByText("Copied")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Copy extraction command" }));
  expect(writeText).toHaveBeenLastCalledWith(s.extractCommand);
  expect(screen.queryByRole("button", { name: "Copy report" })).toBeNull();
  expect(screen.queryByRole("region", { name: "Build report" })).toBeNull();
  expect((await axe(container)).violations).toEqual([]);
});

test("tar shows one size; no normalised list when empty", () => {
  const { container } = show(summary({ format: "tar", bytes: 5 * 1024 }));
  expect(container.querySelector(".build-result__facts")?.textContent).toBe(
    "tar · 2 files, 1 folder · 5.0 KB",
  );
  expect(screen.queryByText("Line endings converted")).toBeNull();
});

test("success with warnings only shows a collapsed Warnings disclosure", () => {
  show(summary({ warnings: [{ ...diag("odd"), severity: "warning" }] }));
  expect(screen.getByRole("region", { name: "Build report" })).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: /Left out/ })).toBeNull();
});

test("files left out: heading, lede, report, Copy report", async () => {
  const user = userWithClipboard();
  const writeText = user.writeText;
  const s = summary({ leftOut: [failure(1)], errorCount: 1, builtIds: ["a", "b"] });
  const { container } = show(s);
  expect(
    screen.getByRole("heading", { level: 2, name: "Created gateway.tar.zst with 1 file left out" }),
  ).toBeInTheDocument();
  expect(container.textContent).toContain(
    "The archive holds 2 of the manifest's 3 files. The rest have errors, listed below.",
  );
  expect(screen.getByRole("heading", { level: 3, name: "Left out of the archive (1)" })).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Copy report" }));
  expect(writeText).toHaveBeenCalledOnce();
  expect(writeText.mock.calls[0][0]).toContain("Left out of the archive:");
  expect(container.textContent).not.toMatch(/\blog\b|saved|report file/i);
  expect((await axe(container)).violations).toEqual([]);
});

test("plural files and manifest-errors-only headings", () => {
  const { unmount } = show(summary({ leftOut: [failure(1), failure(2)], errorCount: 2 }));
  expect(screen.getByRole("heading", { level: 2, name: /with 2 files left out/ })).toBeInTheDocument();
  unmount();
  show(summary({ manifestErrors: [diag("a"), diag("b")], errorCount: 2 }));
  expect(screen.getByRole("heading", { level: 2, name: /with 2 manifest errors/ })).toBeInTheDocument();
  expect(screen.queryByText(/The archive holds/)).toBeNull();
});

test("Show in folder calls onReveal", async () => {
  const onReveal = vi.fn();
  show(summary(), onReveal);
  await userEvent.click(screen.getByRole("button", { name: "Show in folder" }));
  expect(onReveal).toHaveBeenCalled();
});

test("error result uses errorMessage and collapsed details", async () => {
  const { container } = render(
    <BuildResult
      result={{ err: { kind: "SourceMissing", message: "os error 2" } }}
      entries={[]}
      onReveal={() => undefined}
      onDismiss={() => undefined}
    />,
    { wrapper: DevOn },
  );
  expect(
    screen.getByRole("heading", { level: 2, name: "A file is no longer at its assigned location." }),
  ).toBeInTheDocument();
  expect(screen.getByText("Details")).toBeInTheDocument();
  expect((await axe(container)).violations).toEqual([]);
});

test("a rejected clipboard write says Couldn't copy, never Copied", async () => {
  const user = userWithClipboard();
  user.writeText.mockRejectedValue(new Error("denied"));
  show();
  await user.click(screen.getByRole("button", { name: "Copy SHA-256" }));
  expect(await screen.findByText("Couldn't copy")).toBeInTheDocument();
  expect(screen.queryByText("Copied")).toBeNull();
});
