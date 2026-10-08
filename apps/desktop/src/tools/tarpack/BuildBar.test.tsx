import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { beforeEach, expect, test, vi } from "vitest";
import type { Progress } from "../../lib/generated/Progress";
import type { TarpackSession } from "../../lib/generated/TarpackSession";
import { BuildBar } from "./BuildBar";
import { buildable, diag, entry, failure, FORMATS, manifest, session } from "./fixtures";

vi.mock("../../lib/tauri", () => ({ saveFileDialog: vi.fn() }));
import { saveFileDialog } from "../../lib/tauri";

beforeEach(() => vi.resetAllMocks());

function setup(s: TarpackSession, over: { building?: boolean; progress?: Progress | null } = {}) {
  const props = {
    onChooseOutput: vi.fn(),
    onFormatChange: vi.fn(),
    onBuild: vi.fn(),
    onShowErrors: vi.fn(),
    onError: vi.fn(),
  };
  const utils = render(
    <>
      <div id="manifest-error-report" />
      <BuildBar session={s} building={false} progress={null} {...props} {...over} />
    </>,
  );
  return { ...props, ...utils };
}
const create = () => screen.getByRole("button", { name: "Create archive" });

test.each([
  [
    "noManifest",
    session(null, { formats: [...FORMATS], buildBlockedReason: "noManifest" }),
    "Open a manifest first",
  ],
  [
    "noEntries withheld",
    session(manifest({ entries: [], entriesWithheld: true, errors: [diag("x")] }), {
      buildBlockedReason: "noEntries",
    }),
    "No files can be built until the manifest errors are fixed",
  ],
  [
    "noEntries failed",
    session(manifest({ entries: [], failedEntries: [failure(1)] }), { buildBlockedReason: "noEntries" }),
    "Every file in the manifest has errors",
  ],
  [
    "noEntries none",
    session(manifest({ entries: [] }), { buildBlockedReason: "noEntries" }),
    "This manifest lists no files",
  ],
  [
    "one missing",
    session(manifest({ entries: [{ ...entry("a"), status: "missing" }, entry("b")] }), {
      buildBlockedReason: "entriesNotReady",
      readyCount: 0,
      totalCount: 2,
    }),
    "1 chosen file is missing from disk",
  ],
  [
    "two missing",
    session(
      manifest({ entries: [{ ...entry("a"), status: "missing" }, { ...entry("b"), status: "missing" }] }),
      { buildBlockedReason: "entriesNotReady", readyCount: 0, totalCount: 2 },
    ),
    "2 chosen files are missing from disk",
  ],
  [
    "nothing loaded",
    session(manifest({ entries: [entry("a"), entry("b")] }), {
      buildBlockedReason: "nothingLoaded",
      readyCount: 0,
      totalCount: 2,
    }),
    "Add at least one file to build an archive",
  ],
  [
    "noOutput",
    session(manifest({ entries: [{ ...entry("a"), status: "ready" }] }), {
      buildBlockedReason: "noOutput",
      readyCount: 1,
      totalCount: 1,
    }),
    "Choose where to save the archive",
  ],
])("disabled reason: %s", async (_n, s, text) => {
  const { container } = setup(s);
  expect(create()).toBeDisabled();
  expect(screen.getByText(text)).toBeInTheDocument();
  expect(create()).toHaveAccessibleDescription(text);
  expect((await axe(container)).violations).toEqual([]);
});

test("Show errors only with errors, and none when noEntries without errors", () => {
  setup(session(manifest({ entries: [] }), { buildBlockedReason: "noEntries" }));
  expect(screen.queryByRole("button", { name: "Show errors" })).toBeNull();
});

test("ready with failed entries: enabled, note, Show errors before Create", async () => {
  const user = userEvent.setup();
  const s = buildable(manifest({ failedEntries: [failure(1)] }));
  const { onShowErrors, onBuild, container } = setup(s);
  expect(create()).toBeEnabled();
  expect(create()).toHaveAccessibleDescription(
    "1 file will be left out; errors will be listed after the build",
  );
  const show = screen.getByRole("button", { name: "Show errors" });
  expect(show).toHaveAttribute("aria-controls", "manifest-error-report");
  expect(show.compareDocumentPosition(create()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  await user.click(show);
  expect(onShowErrors).toHaveBeenCalled();
  await user.click(create());
  expect(onBuild).toHaveBeenCalledOnce();
  expect((await axe(container)).violations).toEqual([]);
});

test("plural left-out note", () => {
  setup(buildable(manifest({ failedEntries: [failure(1), failure(2)] })));
  expect(
    screen.getByText("2 files will be left out; errors will be listed after the build"),
  ).toBeInTheDocument();
});

test("manifest errors only", () => {
  setup(buildable(manifest({ errors: [diag("a"), diag("b")] })));
  expect(create()).toBeEnabled();
  expect(create()).toHaveAccessibleDescription("Builds with 2 manifest errors");
});

test("one manifest error", () => {
  setup(buildable(manifest({ errors: [diag("a")] })));
  expect(screen.getByText("Builds with 1 manifest error")).toBeInTheDocument();
});

test("nothingLoaded with errors shows reason and note", () => {
  const s = session(manifest({ entries: [entry("a")], failedEntries: [failure(1)] }), {
    buildBlockedReason: "nothingLoaded",
    readyCount: 0,
    totalCount: 1,
  });
  setup(s);
  expect(create()).toBeDisabled();
  expect(create()).toHaveAccessibleDescription(
    "Add at least one file to build an archive 1 file will be left out; errors will be listed after the build",
  );
});

test("a partial build is enabled and says how many files are not included", () => {
  const s = buildable(manifest({ entries: [{ ...entry("a"), status: "ready" }, entry("b"), entry("c")] }), {
    readyCount: 1,
    totalCount: 3,
  });
  setup(s);
  expect(create()).toBeEnabled();
  expect(create()).toHaveAccessibleDescription(
    "Partial archive: 1 of 3 files; 2 without a file will be left out",
  );
});

test("Choose calls the save dialog with the format's filter, then onChooseOutput", async () => {
  const user = userEvent.setup();
  vi.mocked(saveFileDialog).mockResolvedValue("D:\\x\\y.tar.zst");
  const { onChooseOutput } = setup(buildable(manifest()));
  await user.click(screen.getByRole("button", { name: "Choose…" }));
  expect(saveFileDialog).toHaveBeenCalledWith({
    defaultPath: "C:\\out\\gateway.tar.zst",
    filters: [{ name: "zstd archive (.tar.zst)", extensions: ["zst"] }],
  });
  expect(onChooseOutput).toHaveBeenCalledWith("D:\\x\\y.tar.zst");
});

test("Choose falls back to the suggested name; cancel does nothing", async () => {
  const user = userEvent.setup();
  vi.mocked(saveFileDialog).mockResolvedValue(null);
  const { onChooseOutput } = setup(
    buildable(manifest(), { outputPath: null, buildBlockedReason: "noOutput", canBuild: false }),
  );
  await user.click(screen.getByRole("button", { name: "Choose…" }));
  expect(vi.mocked(saveFileDialog).mock.calls[0][0].defaultPath).toBe("gateway.tar.zst");
  expect(onChooseOutput).not.toHaveBeenCalled();
});

test("no manifest: picker shows tar (.tar) and everything is disabled", async () => {
  const { onChooseOutput, onFormatChange } = setup(
    session(null, { formats: [...FORMATS], buildBlockedReason: "noManifest" }),
  );
  expect(screen.getByRole("combobox", { name: "Format" })).toBeDisabled();
  expect(screen.getByRole("combobox", { name: "Format" })).toHaveDisplayValue("tar (.tar)");
  expect(screen.getByRole("button", { name: "Choose…" })).toBeDisabled();
  expect(screen.getByText("not chosen")).toBeInTheDocument();
  expect(saveFileDialog).not.toHaveBeenCalled();
  expect(onChooseOutput).not.toHaveBeenCalled();
  expect(onFormatChange).not.toHaveBeenCalled();
});

test("the output path is a MiddlePath with the full path as title", () => {
  setup(buildable(manifest()));
  expect(screen.getByTitle("C:\\out\\gateway.tar.zst")).toBeInTheDocument();
});

test.each([
  ["writing", { phase: "writing", entryId: "gateway", bytesDone: 40, bytesTotal: 100 } as Progress],
  ["verifying", { phase: "verifying", entryId: null, bytesDone: 10, bytesTotal: 100 } as Progress],
])("building (%s): controls disabled, axe clean", async (_n, progress) => {
  const { container } = setup(buildable(manifest({ entries: [entry("gateway", "gateway.conf")] })), {
    building: true,
    progress,
  });
  expect(create()).toBeDisabled();
  expect(screen.getByRole("combobox", { name: "Format" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Choose…" })).toBeDisabled();
  expect(screen.getByRole("progressbar", { name: "Building archive" })).toBeInTheDocument();
  expect((await axe(container)).violations).toEqual([]);
});

test("saveFileDialog, onChooseOutput, and onFormatChange rejections go to onError", async () => {
  const user = userEvent.setup();
  const s = buildable(manifest({ entries: [entry("a")] }));
  const { onError, onChooseOutput, onFormatChange } = setup(s);
  const e1 = new Error("dialog");
  vi.mocked(saveFileDialog).mockRejectedValueOnce(e1);
  await user.click(screen.getByRole("button", { name: "Choose…" }));
  expect(onError).toHaveBeenCalledWith(e1);
  const e2 = new Error("setOutput");
  vi.mocked(saveFileDialog).mockResolvedValueOnce("C:\\o\\x.tar.zst");
  onChooseOutput.mockRejectedValueOnce(e2);
  await user.click(screen.getByRole("button", { name: "Choose…" }));
  expect(onError).toHaveBeenCalledWith(e2);
  const e3 = new Error("setFormat");
  onFormatChange.mockRejectedValueOnce(e3);
  await user.selectOptions(screen.getByRole("combobox", { name: "Format" }), "tarXz");
  await vi.waitFor(() => expect(onError).toHaveBeenCalledWith(e3));
});

test("while building, Create archive's aria-describedby is omitted", () => {
  const s = buildable(manifest({ entries: [entry("a")], failedEntries: [failure(1)] }));
  setup(s, { building: true });
  expect(create()).not.toHaveAttribute("aria-describedby");
});
