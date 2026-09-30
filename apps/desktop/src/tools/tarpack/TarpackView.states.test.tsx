import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { beforeEach, expect, test, vi } from "vitest";
import { TarpackView } from "./TarpackView";
import { diag, failure, manifest, session } from "./fixtures";

vi.mock("../../lib/tarpack", () => ({
  session: vi.fn(),
  openManifest: vi.fn(),
  reloadManifest: vi.fn(),
  openInEditor: vi.fn(),
  recentManifests: vi.fn(),
  createManifestFromExample: vi.fn(),
  onManifestChanged: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({ openFileDialog: vi.fn(), saveFileDialog: vi.fn(), onDragDrop: vi.fn() }));
import * as tp from "../../lib/tarpack";
import { openFileDialog, saveFileDialog, onDragDrop } from "../../lib/tauri";

let changed: () => void = () => undefined;

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(onDragDrop).mockResolvedValue(() => undefined);
  vi.mocked(tp.recentManifests).mockResolvedValue([]);
  vi.mocked(tp.onManifestChanged).mockImplementation(async (h) => {
    changed = () => h({ path: "x" });
    return () => undefined;
  });
});

const load = async (s = session(manifest())) => {
  vi.mocked(tp.session).mockResolvedValue(s);
  const utils = render(<TarpackView />);
  await screen.findByRole("heading", { level: 1 });
  return utils;
};

test("loading: announcer mounted, hidden text, busy, skeleton delayed", async () => {
  vi.useFakeTimers();
  vi.mocked(tp.session).mockReturnValue(new Promise(() => undefined));
  const { container } = render(<TarpackView />);
  expect(screen.getByRole("status")).toBeInTheDocument();
  expect(screen.getByText("Loading manifest")).toBeInTheDocument();
  expect(container.querySelector("[aria-busy=true]")).not.toBeNull();
  expect(container.querySelector(".skeleton")).toBeNull();
  await act(async () => {
    vi.advanceTimersByTime(200);
  });
  expect(container.querySelector(".skeleton")).not.toBeNull();
  vi.useRealTimers();
  expect((await axe(container)).violations).toEqual([]);
});

test("no-manifest state opens via dialog with a toml filter", async () => {
  const user = userEvent.setup();
  const { container } = await load(session(null));
  expect(screen.getByText("Open a manifest to list the files this package needs.")).toBeInTheDocument();
  expect(screen.getByText(/A manifest is a TOML file/)).toBeInTheDocument();
  expect((await axe(container)).violations).toEqual([]);
  vi.mocked(openFileDialog).mockResolvedValueOnce(null);
  await user.click(screen.getByRole("button", { name: "Open manifest…" }));
  expect(tp.openManifest).not.toHaveBeenCalled();
  vi.mocked(openFileDialog).mockResolvedValueOnce("C:\\m.toml");
  vi.mocked(tp.openManifest).mockResolvedValue(session(manifest()));
  await user.click(screen.getByRole("button", { name: "Open manifest…" }));
  expect(vi.mocked(openFileDialog).mock.calls[1][0]?.filters?.[0].extensions).toEqual(["toml"]);
  expect(tp.openManifest).toHaveBeenCalledWith("C:\\m.toml");
  expect(await screen.findByRole("heading", { level: 1, name: "gateway" })).toBeInTheDocument();
});

test("loaded state has no error region and no axe violations", async () => {
  const { container } = await load();
  expect(screen.queryByRole("region")).toBeNull();
  expect(screen.getByRole("button", { name: "Edit in editor" })).toBeEnabled();
  expect((await axe(container)).violations).toEqual([]);
});

test("errors with entries shown, and withheld", async () => {
  const { container, unmount } = await load(session(manifest({ failedEntries: [failure(1)] })));
  expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("1 error in this manifest");
  expect(screen.getByRole("region", { name: /1 error/ })).toBeVisible();
  expect((await axe(container)).violations).toEqual([]);
  unmount();
  const w = await load(
    session(manifest({ entriesWithheld: true, entries: [], errors: [diag("bad toml")] })),
  );
  expect(screen.getByText(/No files can be listed or built/)).toBeInTheDocument();
  expect((await axe(w.container)).violations).toEqual([]);
});

test("changed on disk banner appears, Reload clears it", async () => {
  const user = userEvent.setup();
  const { container } = await load();
  act(() => changed());
  expect(container.querySelector(".banner--warn")).toHaveTextContent("The manifest changed on disk.");
  vi.mocked(tp.reloadManifest).mockResolvedValue(session(manifest()));
  await user.click(screen.getAllByRole("button", { name: "Reload" })[0]);
  await waitFor(() => expect(container.querySelector(".banner--warn")).toBeNull());
});

test("ManifestUnreadable from Open keeps the session; Details is a disclosure", async () => {
  const user = userEvent.setup();
  await load();
  vi.mocked(openFileDialog).mockResolvedValue("C:\\gone.toml");
  vi.mocked(tp.openManifest).mockRejectedValue({ kind: "ManifestUnreadable", message: "os error 2" });
  await user.click(screen.getByRole("button", { name: "Open…" }));
  const alert = await screen.findByRole("alert");
  expect(alert).toHaveTextContent("The manifest could not be read.");
  expect(screen.getByRole("heading", { level: 1, name: "gateway" })).toBeInTheDocument();
  const summary = screen.getByText("Details");
  expect(screen.getByText("os error 2")).not.toBeVisible();
  // A native <summary> is keyboard operable; jsdom does not synthesize Enter on it, so click.
  await user.click(summary);
  expect(screen.getByText("os error 2")).toBeVisible();
});

test("ManifestUnreadable from Reload", async () => {
  const user = userEvent.setup();
  await load();
  vi.mocked(tp.reloadManifest).mockRejectedValue({ kind: "ManifestUnreadable", message: "gone" });
  await user.click(screen.getByRole("button", { name: "Reload" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("could not be read");
});

test("PathExists from Create from example leaves the no-manifest state", async () => {
  const user = userEvent.setup();
  await load(session(null));
  vi.mocked(saveFileDialog).mockResolvedValue("C:\\exists.toml");
  vi.mocked(tp.createManifestFromExample).mockRejectedValue({ kind: "PathExists", message: "exists" });
  await user.click(screen.getByRole("button", { name: "Create from example…" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("A file already exists there.");
  expect(screen.getByRole("button", { name: "Open manifest…" })).toBeInTheDocument();
});

test("OpenerFailed from Edit in editor and state warning", async () => {
  const user = userEvent.setup();
  const { container } = await load(session(manifest(), { stateWarning: "Saved locations were reset." }));
  expect(container.querySelector(".banner--info")).toHaveTextContent("Saved locations were reset.");
  vi.mocked(tp.openInEditor).mockRejectedValue({ kind: "OpenerFailed", message: "no" });
  await user.click(screen.getByRole("button", { name: "Edit in editor" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Windows could not open it.");
});

test("a rejected first session() shows the no-manifest state with the banner", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.session).mockRejectedValue({ kind: "Io", message: "disk" });
  const { container } = render(<TarpackView />);
  const alert = await screen.findByRole("alert");
  expect(alert).toHaveTextContent("A file could not be read or written.");
  expect(container.querySelector("[aria-busy=true]")).toBeNull();
  expect(container.firstElementChild).toHaveClass("tarpack");
  expect(screen.getByRole("heading", { level: 1, name: "Tar Packager" })).toBeInTheDocument();
  expect(screen.getByText("Open a manifest to list the files this package needs.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Create from example…" })).toBeEnabled();
  expect((await axe(container)).violations).toEqual([]);
  vi.mocked(openFileDialog).mockResolvedValue("C:\\m.toml");
  vi.mocked(tp.openManifest).mockResolvedValue(session(manifest()));
  await user.click(screen.getByRole("button", { name: "Open manifest…" }));
  expect(await screen.findByRole("heading", { level: 1, name: "gateway" })).toBeInTheDocument();
  expect(screen.queryByRole("alert")).toBeNull();
});

test("a non-TarpackError rejection shows the Io copy with the thrown text", async () => {
  const user = userEvent.setup();
  await load();
  vi.mocked(tp.reloadManifest).mockRejectedValue(new Error("ipc down"));
  await user.click(screen.getByRole("button", { name: "Reload" }));
  const alert = await screen.findByRole("alert");
  expect(alert).toHaveTextContent("A file could not be read or written.");
  expect(alert).toHaveTextContent("ipc down");
});

test("a rejected dialog is reported, not left unhandled", async () => {
  const user = userEvent.setup();
  await load(session(null));
  vi.mocked(openFileDialog).mockRejectedValue(new Error("dialog broke"));
  await user.click(screen.getByRole("button", { name: "Open manifest…" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("dialog broke");
});

test("banners render in fixed order and Details open passes axe", async () => {
  const user = userEvent.setup();
  const { container } = await load(session(manifest(), { stateWarning: "Saved locations were reset." }));
  vi.mocked(tp.openInEditor).mockRejectedValue({ kind: "OpenerFailed", message: "no" });
  await user.click(screen.getByRole("button", { name: "Edit in editor" }));
  await screen.findByRole("alert");
  act(() => changed());
  const banners = Array.from(container.querySelectorAll(".banners > .banner")).map((b) =>
    b.className.replace("banner ", ""),
  );
  expect(banners).toEqual(["banner--error", "banner--warn", "banner--info"]);
  expect((await axe(container)).violations).toEqual([]);
  await user.click(screen.getByText("Details"));
  expect((await axe(container)).violations).toEqual([]);
});

test("changed-on-disk banner has no axe violations", async () => {
  const { container } = await load();
  act(() => changed());
  expect((await axe(container)).violations).toEqual([]);
});

test("root section has the tarpack class in loading, no-manifest and loaded states", async () => {
  vi.mocked(tp.session).mockReturnValue(new Promise(() => undefined));
  const a = render(<TarpackView />);
  expect(a.container.firstElementChild).toHaveClass("tool-view", "tarpack");
  a.unmount();
  const b = await load(session(null));
  expect(b.container.firstElementChild).toHaveClass("tarpack");
  b.unmount();
  const c = await load();
  expect(c.container.firstElementChild).toHaveClass("tarpack");
});

test("skeleton has no axe violations after the delay", async () => {
  vi.useFakeTimers();
  vi.mocked(tp.session).mockReturnValue(new Promise(() => undefined));
  const { container } = render(<TarpackView />);
  await act(async () => {
    vi.advanceTimersByTime(200);
  });
  expect(container.querySelector(".skeleton")).not.toBeNull();
  vi.useRealTimers();
  expect((await axe(container)).violations).toEqual([]);
});

test("the same command error twice re-mounts the alert so it is announced again", async () => {
  const user = userEvent.setup();
  await load();
  vi.mocked(tp.reloadManifest).mockRejectedValue({ kind: "ManifestUnreadable", message: "gone" });
  await user.click(screen.getByRole("button", { name: "Reload" }));
  const first = await screen.findByRole("alert");
  await user.click(screen.getByRole("button", { name: "Reload" }));
  await waitFor(() => expect(screen.getByRole("alert")).not.toBe(first));
  expect(first.isConnected).toBe(false);
});
