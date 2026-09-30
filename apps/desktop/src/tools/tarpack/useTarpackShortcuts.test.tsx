import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import type { TarpackSession } from "../../lib/generated/TarpackSession";
import { TarpackView } from "./TarpackView";
import { buildable, entry, failure, manifest, session, summary } from "./fixtures";

vi.mock("../../lib/tarpack", () => ({
  session: vi.fn(),
  build: vi.fn(),
  openManifest: vi.fn(),
  reloadManifest: vi.fn(),
  openInEditor: vi.fn(),
  recentManifests: vi.fn(),
  onManifestChanged: vi.fn(),
  onBuildProgress: vi.fn(),
}));
vi.mock("../../lib/tauri", () => ({ openFileDialog: vi.fn(), saveFileDialog: vi.fn(), onDragDrop: vi.fn() }));
import * as tp from "../../lib/tarpack";
import { onDragDrop, openFileDialog } from "../../lib/tauri";

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(onDragDrop).mockResolvedValue(() => undefined);
  vi.mocked(tp.recentManifests).mockResolvedValue([]);
  vi.mocked(tp.onManifestChanged).mockResolvedValue(() => undefined);
  vi.mocked(tp.onBuildProgress).mockResolvedValue(() => undefined);
  vi.mocked(openFileDialog).mockResolvedValue(null);
});

const ready = buildable(manifest({ entries: [entry("gateway", "gateway.conf")] }));
const withErrors = buildable(
  manifest({ entries: [entry("gateway", "gateway.conf")], failedEntries: [failure(1)] }),
);

async function mount(s: TarpackSession = ready) {
  vi.mocked(tp.session).mockResolvedValue(s);
  const utils = render(<TarpackView />);
  await screen.findByRole("heading", { level: 1 });
  return utils;
}

/** Dispatches a keydown on `window` and reports whether the default was prevented. */
const press = (key: string, mods: KeyboardEventInit = {}) => {
  const ev = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...mods });
  act(() => void document.body.dispatchEvent(ev));
  return ev.defaultPrevented;
};

test("Ctrl+O opens the file dialog, with or without a manifest", async () => {
  await mount(session(null));
  press("o", { ctrlKey: true });
  await waitFor(() => expect(openFileDialog).toHaveBeenCalledTimes(1));
});

test("F5 and Ctrl+R reload and never reload the webview; inactive without a manifest they are still swallowed", async () => {
  vi.mocked(tp.reloadManifest).mockResolvedValue(ready);
  await mount();
  expect(press("F5")).toBe(true);
  await waitFor(() => expect(tp.reloadManifest).toHaveBeenCalledTimes(1));
  expect(press("r", { ctrlKey: true })).toBe(true);
  await waitFor(() => expect(tp.reloadManifest).toHaveBeenCalledTimes(2));
});

test("F5 with no manifest is prevented and does nothing", async () => {
  await mount(session(null));
  expect(press("F5")).toBe(true);
  expect(press("r", { ctrlKey: true })).toBe(true);
  expect(tp.reloadManifest).not.toHaveBeenCalled();
});

test("Ctrl+E edits in the editor only when a manifest is loaded", async () => {
  await mount(session(null));
  press("e", { ctrlKey: true });
  expect(tp.openInEditor).not.toHaveBeenCalled();
});

test("Ctrl+E opens the editor with a manifest", async () => {
  await mount();
  press("e", { ctrlKey: true });
  await waitFor(() => expect(tp.openInEditor).toHaveBeenCalledTimes(1));
});

test("Ctrl+Enter builds when canBuild, and does nothing when it is false", async () => {
  vi.mocked(tp.build).mockResolvedValue(summary());
  const { unmount } = await mount(buildable(manifest(), { canBuild: false, buildBlockedReason: "noOutput" }));
  press("Enter", { ctrlKey: true });
  expect(tp.build).not.toHaveBeenCalled();
  unmount();
  await mount();
  press("Enter", { ctrlKey: true });
  await waitFor(() => expect(tp.build).toHaveBeenCalledWith(false));
  expect(await screen.findByRole("heading", { level: 2, name: /Created/ })).toBeInTheDocument();
});

test("Ctrl+Enter builds a canBuild session that has errors, with no dialog", async () => {
  vi.mocked(tp.build).mockResolvedValue(summary());
  await mount(withErrors);
  press("Enter", { ctrlKey: true });
  await waitFor(() => expect(tp.build).toHaveBeenCalledWith(false));
  expect(screen.queryByRole("dialog")).toBeNull();
});

test("F8 moves focus to the error report when there are errors, and does nothing otherwise", async () => {
  const { unmount } = await mount(withErrors);
  press("F8");
  expect(document.getElementById("manifest-error-report")).toHaveFocus();
  unmount();
  await mount();
  const before = document.activeElement;
  press("F8");
  expect(document.activeElement).toBe(before);
});

test("Escape dismisses the build result, but never collapses the error report", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockResolvedValue(summary());
  await mount(withErrors);
  await user.click(screen.getByRole("button", { name: "Create archive" }));
  await screen.findByRole("heading", { level: 2, name: /Created/ });
  press("Escape");
  await waitFor(() => expect(screen.queryByRole("heading", { level: 2, name: /Created/ })).toBeNull());
  expect(screen.getByRole("region", { name: /error/ })).toBeVisible();
  press("Escape");
  expect(screen.getByRole("region", { name: /error/ })).toBeVisible();
});

test("shortcuts are ignored while a build is running", async () => {
  let finish!: (s: ReturnType<typeof summary>) => void;
  vi.mocked(tp.build).mockReturnValue(new Promise((r) => (finish = r)));
  vi.mocked(tp.reloadManifest).mockResolvedValue(ready);
  await mount();
  press("Enter", { ctrlKey: true });
  await waitFor(() => expect(tp.build).toHaveBeenCalledTimes(1));
  press("Enter", { ctrlKey: true });
  press("o", { ctrlKey: true });
  expect(press("F5")).toBe(true);
  press("e", { ctrlKey: true });
  expect(tp.build).toHaveBeenCalledTimes(1);
  expect(openFileDialog).not.toHaveBeenCalled();
  expect(tp.reloadManifest).not.toHaveBeenCalled();
  expect(tp.openInEditor).not.toHaveBeenCalled();
  await act(async () => finish(summary()));
});

test("shortcuts are ignored while the replace dialog is open", async () => {
  vi.mocked(tp.build).mockRejectedValue({ kind: "OutputExists", message: "exists" });
  await mount();
  press("Enter", { ctrlKey: true });
  await screen.findByRole("dialog");
  press("Enter", { ctrlKey: true });
  press("o", { ctrlKey: true });
  expect(tp.build).toHaveBeenCalledTimes(1);
  expect(openFileDialog).not.toHaveBeenCalled();
});

test("buttons carry their shortcut hints", async () => {
  await mount(withErrors);
  expect(screen.getByRole("button", { name: "Create archive" })).toHaveAttribute(
    "aria-keyshortcuts",
    "Control+Enter",
  );
  const bar = document.querySelector(".build-bar")!;
  const show = Array.from(bar.querySelectorAll("button")).find((b) => b.textContent === "Show errors")!;
  expect(show).toHaveAttribute("aria-keyshortcuts", "F8");
  expect(screen.getByRole("button", { name: "Reload" })).toHaveAttribute("aria-keyshortcuts", "F5 Control+R");
  expect(screen.getAllByRole("button", { name: "Edit in editor" })[0]).toHaveAttribute("aria-keyshortcuts", "Control+E");
});

test("tab order through the build bar: Choose…, Format, Show errors, Create archive", async () => {
  const user = userEvent.setup();
  await mount(withErrors);
  screen.getByRole("button", { name: "Choose…" }).focus();
  await user.tab();
  expect(screen.getByRole("combobox", { name: "Format" })).toHaveFocus();
  await user.tab();
  expect(document.activeElement).toHaveTextContent("Show errors");
  expect(document.activeElement?.closest(".build-bar")).not.toBeNull();
  await user.tab();
  expect(screen.getByRole("button", { name: "Create archive" })).toHaveFocus();
});

test.each([
  ["Shift+F5", "F5", { shiftKey: true }],
  ["Ctrl+F5", "F5", { ctrlKey: true }],
  ["Ctrl+Shift+R", "R", { ctrlKey: true, shiftKey: true }],
])("%s never reloads the webview and never runs Reload manifest", async (_n, key, mods) => {
  await mount();
  expect(press(key, mods)).toBe(true);
  expect(tp.reloadManifest).not.toHaveBeenCalled();
});

test("reload chords are prevented with no manifest too", async () => {
  await mount(session(null));
  expect(press("F5", { shiftKey: true })).toBe(true);
  expect(press("F5", { ctrlKey: true })).toBe(true);
  expect(press("R", { ctrlKey: true, shiftKey: true })).toBe(true);
  expect(tp.reloadManifest).not.toHaveBeenCalled();
});

test("reload chords are prevented during a build", async () => {
  vi.mocked(tp.build).mockReturnValue(new Promise(() => undefined));
  await mount();
  press("Enter", { ctrlKey: true });
  await waitFor(() => expect(tp.build).toHaveBeenCalledTimes(1));
  expect(press("F5", { shiftKey: true })).toBe(true);
  expect(press("F5", { ctrlKey: true })).toBe(true);
  expect(press("R", { ctrlKey: true, shiftKey: true })).toBe(true);
  expect(tp.reloadManifest).not.toHaveBeenCalled();
});

test("Escape while the shortcuts popover is open neither dismisses a result nor is prevented", async () => {
  const user = userEvent.setup();
  vi.mocked(tp.build).mockResolvedValue(summary());
  await mount();
  await user.click(screen.getByRole("button", { name: "Create archive" }));
  await screen.findByRole("heading", { level: 2, name: /Created/ });
  await user.click(screen.getByRole("button", { name: "Keyboard shortcuts" }));
  const pop = document.querySelector("[popover]") as HTMLElement;
  await waitFor(() => expect(pop.dataset.open).toBe("true"));
  screen.getByRole("button", { name: "Create archive" }).focus();
  expect(press("Escape")).toBe(false);
  expect(screen.getByRole("heading", { level: 2, name: /Created/ })).toBeInTheDocument();
  // Once the popover is closed, Escape dismisses as before.
  act(() => pop.hidePopover?.());
  await waitFor(() => expect(pop.dataset.open).toBe("false"));
  press("Escape");
  await waitFor(() => expect(screen.queryByRole("heading", { level: 2, name: /Created/ })).toBeNull());
});
