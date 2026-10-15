import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { beforeEach, expect, test, vi } from "vitest";
import type { PipelineSession } from "../../lib/generated/PipelineSession";
import type { StepId } from "../../lib/generated/StepId";
import type { StepView } from "../../lib/generated/StepView";
import { cleanPath, PipelineView } from "./PipelineView";

vi.mock("../../lib/pipeline", () => ({
  session: vi.fn(),
  setDirectory: vi.fn(),
  runStep: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({ openDirectoryDialog: vi.fn() }));
import * as pl from "../../lib/pipeline";
import { openDirectoryDialog } from "../../lib/tauri";

const DIR = "C:\\jobs";
const IDS: StepId[] = ["Verify", "RunBatch", "Schedule", "Tarpack"];
const notRun = (id: StepId): StepView => ({ id, report: null, error: null });
const empty: PipelineSession = {
  directory: null,
  steps: IDS.map(notRun),
  running: null,
  canRun: false,
};
const ready: PipelineSession = {
  ...empty,
  directory: { path: DIR, error: null, subdirectories: 3 },
  canRun: true,
};
const withStep = (s: PipelineSession, view: StepView): PipelineSession => ({
  ...s,
  steps: s.steps.map((v) => (v.id === view.id ? view : v)),
});
const passed = (id: StepId): StepView => ({
  id,
  report: { summary: "3 folders checked", issues: [], passed: true },
  error: null,
});
const notImplemented = (id: StepId): StepView => ({
  id,
  report: null,
  error: {
    kind: "StepNotImplemented",
    message: "this step is not implemented yet",
  },
});

beforeEach(() => {
  vi.resetAllMocks();
});

const load = async (s: PipelineSession) => {
  vi.mocked(pl.session).mockResolvedValue(s);
  const utils = render(<PipelineView />);
  await screen.findByRole("textbox", { name: "Folder" });
  return utils;
};
const step = (title: string) => screen.getByRole("listitem", { name: title });

test("cleanPath trims and drops Explorer's quotes", () => {
  expect(cleanPath('  "C:\\a b\\jobs"  ')).toBe("C:\\a b\\jobs");
  expect(cleanPath("   ")).toBeNull();
});

test("lists the four steps in order and blocks running without a folder", async () => {
  const { container } = await load(empty);
  const items = within(
    screen.getByRole("list", { name: "Steps" }),
  ).getAllByRole("listitem");
  expect(
    items.map((li) => within(li).getByRole("heading").textContent),
  ).toEqual([
    "Check required files",
    "Run batch files",
    "Schedule",
    "Pack archives",
  ]);
  expect(screen.getByRole("button", { name: "Run all…" })).toBeDisabled();
  expect(screen.getByText("Choose a folder.")).toBeInTheDocument();
  expect((await axe(container)).violations).toEqual([]);
});

test("browsing sets the folder and shows its subfolder count", async () => {
  await load(empty);
  vi.mocked(openDirectoryDialog).mockResolvedValue(DIR);
  vi.mocked(pl.setDirectory).mockResolvedValue(ready);
  await userEvent.click(
    screen.getByRole("button", { name: "Browse for the folder" }),
  );
  expect(pl.setDirectory).toHaveBeenCalledWith(DIR);
  expect(await screen.findByText("3 subfolders")).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "Folder" })).toHaveValue(DIR);
  expect(screen.getByRole("button", { name: "Run all…" })).toBeEnabled();
});

test("a typed bad path is shown with its error", async () => {
  await load(empty);
  vi.mocked(pl.setDirectory).mockResolvedValue({
    ...empty,
    directory: {
      path: "jobs",
      error: { kind: "NotAbsolute", message: "jobs is not a full path" },
      subdirectories: 0,
    },
  });
  await userEvent.type(
    screen.getByRole("textbox", { name: "Folder" }),
    "jobs{Enter}",
  );
  expect(pl.setDirectory).toHaveBeenCalledWith("jobs");
  const box = screen.getByRole("textbox", { name: "Folder" });
  await waitFor(() => expect(box).toHaveAttribute("aria-invalid", "true"));
  expect(box).toHaveAccessibleDescription(/full path/);
});

test("the check step runs without confirming and shows its result", async () => {
  await load(ready);
  vi.mocked(pl.runStep).mockResolvedValue(withStep(ready, passed("Verify")));
  await userEvent.click(
    screen.getByRole("button", { name: "Run Check required files" }),
  );
  expect(pl.runStep).toHaveBeenCalledWith("Verify", DIR);
  const item = step("Check required files");
  expect(await within(item).findByText("Passed")).toBeInTheDocument();
  expect(within(item).getByText("3 folders checked")).toBeInTheDocument();
  await waitFor(() => expect(item).toHaveFocus());
});

test("issues are listed with their subfolder", async () => {
  await load(ready);
  vi.mocked(pl.runStep).mockResolvedValue(
    withStep(ready, {
      id: "Verify",
      report: {
        summary: "3 folders checked",
        issues: [{ subdirectory: "job-2", message: "run.bat is missing" }],
        passed: false,
      },
      error: null,
    }),
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Run Check required files" }),
  );
  const issues = await screen.findByRole("list", {
    name: "Check required files problems",
  });
  expect(issues).toHaveTextContent("job-2: run.bat is missing");
  expect(
    within(step("Check required files")).getByText("1 problem"),
  ).toBeInTheDocument();
});

test("the batch step asks first, and cancelling runs nothing", async () => {
  await load(ready);
  await userEvent.click(
    screen.getByRole("button", { name: "Run Run batch files" }),
  );
  const dialog = screen.getByRole("dialog", { name: "Run Run batch files?" });
  expect(dialog).toHaveTextContent(DIR);
  await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
  expect(pl.runStep).not.toHaveBeenCalled();

  vi.mocked(pl.runStep).mockResolvedValue(
    withStep(ready, notImplemented("RunBatch")),
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Run Run batch files" }),
  );
  await userEvent.click(
    within(
      screen.getByRole("dialog", { name: "Run Run batch files?" }),
    ).getByRole("button", { name: "Run" }),
  );
  expect(pl.runStep).toHaveBeenCalledWith("RunBatch", DIR);
  expect(
    await within(step("Run batch files")).findByText("Not implemented"),
  ).toBeInTheDocument();
});

test("run all goes in order and stops at the first step that does not pass", async () => {
  await load(ready);
  let s = ready;
  vi.mocked(pl.runStep).mockImplementation(async (id) => {
    s = withStep(s, id === "Verify" ? passed(id) : notImplemented(id));
    return s;
  });
  await userEvent.click(screen.getByRole("button", { name: "Run all…" }));
  await userEvent.click(
    within(screen.getByRole("dialog", { name: "Run all steps?" })).getByRole(
      "button",
      { name: "Run" },
    ),
  );
  await waitFor(() => expect(step("Run batch files")).toHaveFocus());
  expect(vi.mocked(pl.runStep).mock.calls).toEqual([
    ["Verify", DIR],
    ["RunBatch", DIR],
  ]);
  expect(within(step("Schedule")).getByText("Not run")).toBeInTheDocument();
});

test("a step that cannot start shows an error banner", async () => {
  await load(ready);
  vi.mocked(pl.runStep).mockRejectedValue({
    kind: "DirectoryChanged",
    message: "changed",
  });
  await userEvent.click(
    screen.getByRole("button", { name: "Run Check required files" }),
  );
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "The chosen folder changed",
  );
});
