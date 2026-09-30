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
  setFormat: vi.fn(),
  revealOutput: vi.fn(),
  reloadManifest: vi.fn(),
  recentManifests: vi.fn(),
  onManifestChanged: vi.fn(),
  onBuildProgress: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({ openFileDialog: vi.fn(), saveFileDialog: vi.fn(), onDragDrop: vi.fn() }));
import * as tp from "../../lib/tarpack";
import { onDragDrop, saveFileDialog } from "../../lib/tauri";

let emit: (p: Progress) => void = () => undefined;
const unlisten = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(onDragDrop).mockResolvedValue(() => undefined);
  vi.mocked(tp.recentManifests).mockResolvedValue([]);
  vi.mocked(tp.onManifestChanged).mockResolvedValue(() => undefined);
  vi.mocked(tp.onBuildProgress).mockImplementation(async (h) => {
    emit = h;
    return unlisten;
  });
});

const ready = buildable(manifest({ entries: [entry("gateway", "gateway.conf")] }));
const announcer = () => screen.getAllByRole("status")[0];
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
  expect(await screen.findByText("The manifest changed on disk.")).toBeInTheDocument();
  expect(screen.queryByRole("heading", { level: 2 })).toBeNull();
  const banner = screen.getByText("The manifest changed on disk.").closest(".banner") as HTMLElement;
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
  expect(bar).toHaveAttribute("aria-valuetext", "Writing gateway.conf, 60%");
  expect(screen.getByText(/gateway\.conf/, { selector: ".build-progress__file" })).toBeInTheDocument();
  act(() => emit(ev("writing", null, 100)));
  act(() => emit(ev("verifying", "gateway", 10)));
  expect(screen.getByText(/Step 2 of 2 · Verifying/)).toBeInTheDocument();
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
