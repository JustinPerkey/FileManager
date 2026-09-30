import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { beforeEach, expect, test, vi } from "vitest";
import type { Progress } from "../../lib/generated/Progress";
import type { TarpackSession } from "../../lib/generated/TarpackSession";
import { TarpackView } from "./TarpackView";
import { buildable, diag, entry, failure, manifest, summary } from "./fixtures";

vi.mock("../../lib/tarpack", () => ({
  session: vi.fn(),
  build: vi.fn(),
  setOutput: vi.fn(),
  clear: vi.fn(),
  setFormat: vi.fn(),
  revealOutput: vi.fn(),
  reloadManifest: vi.fn(),
  recentManifests: vi.fn(),
  onManifestChanged: vi.fn(),
  onBuildProgress: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({ openFileDialog: vi.fn(), saveFileDialog: vi.fn(), onDragDrop: vi.fn() }));
import * as tp from "../../lib/tarpack";
import { onDragDrop, openFileDialog, saveFileDialog } from "../../lib/tauri";

let emit: (p: Progress) => void = () => undefined;
const unlisten = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(onDragDrop).mockResolvedValue(() => undefined);
  vi.mocked(openFileDialog).mockResolvedValue(null);
  vi.mocked(tp.recentManifests).mockResolvedValue([]);
  vi.mocked(tp.onManifestChanged).mockResolvedValue(() => undefined);
  vi.mocked(tp.onBuildProgress).mockImplementation(async (h) => {
    emit = h;
    return unlisten;
  });
});

const ready = buildable(manifest({ entries: [entry("gateway", "gateway.conf")] }));
const announcer = () => screen.getByTestId("tarpack-announcer");
const create = () => screen.getByRole("button", { name: "Create archive" });

async function mount(s: TarpackSession = ready) {
  vi.mocked(tp.session).mockResolvedValue(s);
  const utils = render(<TarpackView />);
  await screen.findByRole("heading", { level: 1 });
  return utils;
}
const rejectWith = (kind: string, extra: object = {}) => ({ kind, message: `technical ${kind}`, ...extra });
const ev = (phase: Progress["phase"], entryId: string | null, bytesDone: number): Progress => ({
  phase,
  entryId,
  bytesDone,
  bytesTotal: 100,
});

test("happy path: build(false), result, announcement, Show in folder", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockResolvedValue(summary());
  const { container } = await mount();
  await user.click(create());
  expect(tp.build).toHaveBeenCalledWith(false);
  expect(
    await screen.findByRole("heading", { level: 2, name: "Created gateway.tar.zst" }),
  ).toBeInTheDocument();
  await waitFor(() => expect(announcer()).toHaveTextContent("Created gateway.tar.zst."));
  expect(unlisten).toHaveBeenCalled();
  expect(create()).toHaveFocus();
  await user.click(screen.getByRole("button", { name: "Show in folder" }));
  expect(tp.revealOutput).toHaveBeenCalled();
  expect((await axe(container)).violations).toEqual([]);
});

test("setFormat re-renders the output path from the returned session", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.setFormat).mockResolvedValue({
    ...ready,
    format: "tarXz",
    outputPath: "C:\\out\\gateway.tar.xz",
  });
  await mount();
  await user.selectOptions(screen.getByRole("combobox", { name: "Format" }), "tarXz");
  expect(tp.setFormat).toHaveBeenCalledWith("tarXz");
  expect(await screen.findByTitle("C:\\out\\gateway.tar.xz")).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "Format" })).toHaveValue("tarXz");
});

test("setOutput returning a switched format announces it", async () => {
  const user = userEvent.setup();
  vi.mocked(saveFileDialog).mockResolvedValue("C:\\out\\other.tar.xz");
  vi.mocked(tp.setOutput).mockResolvedValue({
    ...ready,
    format: "tarXz",
    outputPath: "C:\\out\\other.tar.xz",
  });
  await mount();
  await user.click(screen.getByRole("button", { name: "Choose…" }));
  expect(tp.setOutput).toHaveBeenCalledWith("C:\\out\\other.tar.xz");
  expect(await screen.findByText("Format changed to xz to match the file name.")).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "Format" })).toHaveValue("tarXz");
});

test("OutputExists then Cancel makes no further call", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockRejectedValue(rejectWith("OutputExists"));
  const { container } = await mount();
  await user.click(create());
  const dialog = await screen.findByRole("heading", { name: "Replace gateway.tar.zst?" });
  expect(container.querySelector("dialog")).toHaveTextContent(
    "A file with this name already exists in C:\\out. Replacing it can't be undone.",
  );
  expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
  expect(dialog).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Cancel" }));
  expect(tp.build).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("heading", { level: 2, name: /Created/ })).toBeNull();
  await waitFor(() => expect(create()).toHaveFocus());
});

test("OutputExists then Replace calls build(true)", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockRejectedValueOnce(rejectWith("OutputExists")).mockResolvedValueOnce(summary());
  await mount();
  await user.click(create());
  await user.click(await screen.findByRole("button", { name: "Replace" }));
  expect(tp.build).toHaveBeenNthCalledWith(1, false);
  expect(tp.build).toHaveBeenNthCalledWith(2, true);
  expect(
    await screen.findByRole("heading", { level: 2, name: "Created gateway.tar.zst" }),
  ).toBeInTheDocument();
});

test("ManifestChangedOnDisk shows the banner with Reload, not a result", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockRejectedValue(rejectWith("ManifestChangedOnDisk"));
  vi.mocked(tp.reloadManifest).mockResolvedValue(ready);
  await mount();
  await user.click(create());
  await waitFor(() => expect(document.querySelector(".banner")).not.toBeNull());
  const banner = document.querySelector(".banner") as HTMLElement;
  expect(within(banner).getByText("The manifest changed on disk.")).toBeInTheDocument();
  expect(screen.queryByRole("heading", { level: 2 })).toBeNull();
  expect(announcer()).toHaveTextContent(/^(The manifest changed on disk\.)?$/);
  await user.click(within(banner).getByRole("button", { name: "Reload" }));
  expect(tp.reloadManifest).toHaveBeenCalled();
});

test("SourceMissing names the file; without entryId says A file", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build)
    .mockRejectedValueOnce(rejectWith("SourceMissing", { entryId: "gateway" }))
    .mockRejectedValueOnce(rejectWith("SourceMissing"));
  await mount();
  await user.click(create());
  expect(
    await screen.findByRole("heading", {
      level: 2,
      name: "gateway.conf is no longer at its assigned location.",
    }),
  ).toBeInTheDocument();
  await user.click(create());
  expect(
    await screen.findByRole("heading", { level: 2, name: "A file is no longer at its assigned location." }),
  ).toBeInTheDocument();
});

test.each([
  [
    "SourceChanged",
    "gateway.conf changed while the archive was being written. Nothing was saved. Try again.",
  ],
  [
    "VerifyFailed",
    "The archive failed its check after writing, so it was not saved. Any existing file was left unchanged. Try again.",
  ],
])("%s shows its message and details; axe clean", async (kind, text) => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockRejectedValue(rejectWith(kind, { entryId: "gateway" }));
  const { container } = await mount();
  await user.click(create());
  expect(await screen.findByRole("heading", { level: 2, name: text })).toBeInTheDocument();
  await user.click(screen.getByText("Details"));
  expect(screen.getByText(`technical ${kind}`)).toBeVisible();
  expect((await axe(container)).violations).toEqual([]);
});

test("progress: two phases, reset, Finishing, then the result", async () => {
  const user = userEvent.setup();
  let done!: (s: ReturnType<typeof summary>) => void;
  vi.mocked(tp.build).mockReturnValue(new Promise((r) => (done = r)));
  const { container } = await mount();
  await user.click(create());
  const bar = await screen.findByRole("progressbar", { name: "Building archive" });
  expect(screen.getByText(/Step 1 of 2 · Writing/)).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "Format" })).toBeDisabled();
  expect(create()).toBeDisabled();

  await waitFor(() => expect(tp.onBuildProgress).toHaveBeenCalled());
  act(() => emit(ev("writing", "gateway", 40)));
  expect(bar).toHaveAttribute("aria-valuetext", "Writing gateway.conf, 40%");
  act(() => emit(ev("writing", null, 60)));
  expect(bar).toHaveAttribute("aria-valuetext", "Writing, 60%");
  expect(screen.getByText(/gateway\.conf/, { selector: ".build-progress__file" })).toBeInTheDocument();
  act(() => emit(ev("writing", null, 100)));
  act(() => emit(ev("verifying", null, 5)));
  expect(screen.getByText(/Step 2 of 2 · Verifying/)).toBeInTheDocument();
  expect(bar).toHaveAttribute("aria-valuetext", "Verifying, 5%");
  expect(container.querySelector(".build-progress__file")).toBeNull();
  act(() => emit(ev("verifying", "gateway", 10)));
  expect(bar).toHaveAttribute("aria-valuetext", "Verifying gateway.conf, 10%");
  expect(bar).toHaveAttribute("aria-valuenow", "10");
  expect(container.querySelector(".build-progress [role=status]")).toHaveTextContent("Verifying the archive");
  act(() => emit(ev("verifying", null, 100)));
  expect(screen.getByText("Finishing…")).toBeInTheDocument();
  expect(bar).toHaveAttribute("aria-valuenow", "100");
  expect((await axe(container)).violations).toEqual([]);

  await act(async () => done(summary()));
  expect(
    await screen.findByRole("heading", { level: 2, name: "Created gateway.tar.zst" }),
  ).toBeInTheDocument();
  expect(screen.queryByRole("progressbar")).toBeNull();
});

test("a rejection mid-verify stops the progress and shows the error", async () => {
  const user = userEvent.setup();
  let fail!: (e: unknown) => void;
  vi.mocked(tp.build).mockReturnValue(new Promise((_, r) => (fail = r)));
  await mount();
  await user.click(create());
  await screen.findByRole("progressbar");
  await waitFor(() => expect(tp.onBuildProgress).toHaveBeenCalled());
  act(() => emit(ev("verifying", "gateway", 50)));
  await act(async () => fail(rejectWith("VerifyFailed")));
  expect(screen.queryByRole("progressbar")).toBeNull();
  expect(await screen.findByRole("heading", { level: 2, name: /failed its check/ })).toBeInTheDocument();
});

test("OpenerFailed from Show in folder is a banner", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockResolvedValue(summary());
  vi.mocked(tp.revealOutput).mockRejectedValue(rejectWith("OpenerFailed"));
  await mount();
  await user.click(create());
  await user.click(await screen.findByRole("button", { name: "Show in folder" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Windows could not open it.");
});

test("Show errors in the bar focuses the error report", async () => {
  const user = userEvent.setup();
  await mount(buildable(manifest({ entries: [entry("gateway")], failedEntries: [failure(1)] })));
  const bar = document.querySelector(".build-bar") as HTMLElement;
  await user.click(within(bar).getByRole("button", { name: "Show errors" }));
  expect(document.getElementById("manifest-error-report")).toHaveFocus();
});

test("building with errors: no dialog, build(false) once, left-out result and announcement", async () => {
  const user = userEvent.setup();
  const s = buildable(manifest({ entries: [entry("gateway")], failedEntries: [failure(1)] }));
  vi.mocked(tp.build).mockResolvedValue(
    summary({
      leftOut: [failure(1, { errors: [diag("missing dir", 6, 3)] })],
      errorCount: 1,
      builtIds: ["gateway"],
    }),
  );
  await mount(s);
  await user.click(create());
  expect(document.querySelector("dialog")).not.toHaveAttribute("open");
  expect(tp.build).toHaveBeenCalledTimes(1);
  expect(tp.build).toHaveBeenCalledWith(false);
  expect(
    await screen.findByRole("heading", { level: 2, name: "Created gateway.tar.zst with 1 file left out" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("region", { name: "Build report" })).toHaveTextContent("missing dir");
  await waitFor(() =>
    expect(announcer()).toHaveTextContent("Created gateway.tar.zst. 1 file was left out because of errors."),
  );
});

test.each([
  ["SourceMissing", { entryId: "gateway" }, "gateway.conf is no longer at its assigned location."],
  ["VerifyFailed", {}, /failed its check after writing/],
])("a failed %s build announces its errorMessage", async (kind, extra, text) => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockRejectedValue(rejectWith(kind, extra));
  await mount();
  await user.click(create());
  await waitFor(() => expect(announcer()).toHaveTextContent(text));
  expect(document.querySelector("[role=alert]")).toBeNull();
});

test.each(["OutputExists", "ManifestChangedOnDisk"])("%s announces no error message", async (kind) => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockRejectedValue(rejectWith(kind));
  await mount();
  await user.click(create());
  await waitFor(() => expect(create()).toBeEnabled());
  await new Promise((r) => setTimeout(r, 50));
  expect(announcer()).not.toHaveTextContent(/already exists|Reload it, then build again/);
});

test("onBuildProgress rejecting: the build still runs, no progressbar, unavailable text", async () => {
  const user = userEvent.setup();
  let done!: (s: ReturnType<typeof summary>) => void;
  vi.mocked(tp.onBuildProgress).mockRejectedValue(new Error("no events"));
  vi.mocked(tp.build).mockReturnValue(new Promise((r) => (done = r)));
  await mount();
  await user.click(create());
  expect(await screen.findByText("Creating the archive… (progress is unavailable)")).toBeInTheDocument();
  expect(screen.queryByRole("progressbar")).toBeNull();
  expect(create()).toHaveAttribute("aria-busy", "true");
  await act(async () => done(summary()));
  expect(
    await screen.findByRole("heading", { level: 2, name: "Created gateway.tar.zst" }),
  ).toBeInTheDocument();
  expect(screen.queryByText(/progress is unavailable/)).toBeNull();
});

test("during a build, header and table controls are disabled; describedby has no dangling ids", async () => {
  const user = userEvent.setup();
  let done!: (s: ReturnType<typeof summary>) => void;
  vi.mocked(tp.build).mockReturnValue(new Promise((r) => (done = r)));
  const assigned = { ...entry("gateway", "gateway.conf"), assigned: "C:\\src\\gateway.conf", status: "ready" as const };
  await mount(buildable(manifest({ entries: [assigned] })));
  const names = [/^Reload/, /^Edit in editor/, /^Browse…/, /^Clear/];
  for (const name of names) expect(screen.getByRole("button", { name })).toBeEnabled();
  await user.click(create());
  await screen.findByRole("progressbar");
  for (const name of names) expect(screen.getByRole("button", { name })).toBeDisabled();
  expect(create().getAttribute("aria-describedby")).toBeNull();
  await act(async () => done(summary()));
  await screen.findByRole("heading", { level: 2, name: /Created/ });
  for (const name of names) expect(screen.getByRole("button", { name })).toBeEnabled();
});

test("describedby ids all exist while building with errors", async () => {
  const user = userEvent.setup();
  let done!: (s: ReturnType<typeof summary>) => void;
  vi.mocked(tp.build).mockReturnValue(new Promise((r) => (done = r)));
  await mount(buildable(manifest({ entries: [entry("gateway", "gateway.conf")], failedEntries: [failure(1)] })));
  expect(create().getAttribute("aria-describedby")).toBeTruthy();
  await user.click(create());
  await screen.findByRole("progressbar");
  const ids = (create().getAttribute("aria-describedby") ?? "").split(" ").filter(Boolean);
  for (const id of ids) expect(document.getElementById(id)).not.toBeNull();
  await act(async () => done(summary()));
  await screen.findByRole("heading", { level: 2, name: /Created/ });
});

test("a saveFileDialog rejection from Choose shows the error banner", async () => {
  const user = userEvent.setup();
  vi.mocked(saveFileDialog).mockRejectedValue(rejectWith("Io"));
  await mount();
  await user.click(screen.getByRole("button", { name: "Choose…" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("A file could not be read or written.");
});

test("setOutput and setFormat rejections show the error banner", async () => {
  const user = userEvent.setup();
  vi.mocked(saveFileDialog).mockResolvedValue("C:\\out\\x.tar.zst");
  vi.mocked(tp.setOutput).mockRejectedValue(rejectWith("Io"));
  vi.mocked(tp.setFormat).mockRejectedValue(rejectWith("NoManifest"));
  await mount();
  await user.click(screen.getByRole("button", { name: "Choose…" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("A file could not be read or written.");
  await user.selectOptions(screen.getByRole("combobox", { name: "Format" }), "tarXz");
  expect(await screen.findByText("Open a manifest first.")).toBeInTheDocument();
});

test("the replace dialog shows a lossy folder as is", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockRejectedValue(rejectWith("OutputExists"));
  await mount({ ...ready, outputPath: "C:\\out\\b\uFFFDd\\gateway.tar.zst" });
  await user.click(create());
  await screen.findByRole("heading", { name: "Replace gateway.tar.zst?" });
  expect(document.querySelector("dialog")).toHaveTextContent("already exists in C:\\out\\b\uFFFDd.");
});

test("after a build, focus moves to Create archive only if it was lost", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockResolvedValue(summary());
  await mount();
  await user.click(create());
  await screen.findByRole("heading", { level: 2, name: /Created/ });
  expect(create()).toHaveFocus();
  // Dismiss loses focus (the button unmounts): it returns to Create archive.
  await user.click(screen.getByRole("button", { name: "Dismiss result" }));
  await waitFor(() => expect(create()).toHaveFocus());
});

test("focus that is elsewhere when a build settles stays there", async () => {
  const user = userEvent.setup();
  let done!: (s: ReturnType<typeof summary>) => void;
  vi.mocked(tp.build).mockReturnValue(new Promise((r) => (done = r)));
  await mount(buildable(manifest({ entries: [entry("gateway", "gateway.conf")], failedEntries: [failure(1)] })));
  await user.click(create());
  await screen.findByRole("progressbar");
  const report = document.getElementById("manifest-error-report") as HTMLElement;
  act(() => report.focus());
  expect(report).toHaveFocus();
  await act(async () => done(summary()));
  await screen.findByRole("heading", { level: 2, name: /Created/ });
  expect(report).toHaveFocus();
  expect(create()).not.toHaveFocus();
});

test("row keys do nothing during a build, and work again after it", async () => {
  const user = userEvent.setup();
  let done!: (s: ReturnType<typeof summary>) => void;
  vi.mocked(tp.build).mockReturnValue(new Promise((r) => (done = r)));
  const assigned = { ...entry("gateway", "gateway.conf"), assigned: "C:\\src\\gateway.conf", status: "ready" as const };
  await mount(buildable(manifest({ entries: [assigned] })));
  const row = screen.getAllByRole("row")[1];
  act(() => row.focus());
  await user.click(create());
  await screen.findByRole("progressbar");
  act(() => row.focus());
  await user.keyboard("{Enter}");
  await user.keyboard("{Delete}");
  expect(openFileDialog).not.toHaveBeenCalled();
  expect(tp.clear).not.toHaveBeenCalled();
  await act(async () => done(summary()));
  await screen.findByRole("heading", { level: 2, name: /Created/ });
  act(() => row.focus());
  await user.keyboard("{Enter}");
  await waitFor(() => expect(openFileDialog).toHaveBeenCalledTimes(1));
});
