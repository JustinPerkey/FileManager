import { act, render, screen, waitFor } from "@testing-library/react";
import { axe } from "vitest-axe";
import { beforeEach, expect, test, vi } from "vitest";
import { DropZone } from "./DropZone";

vi.mock("../lib/tauri", () => ({ onDragDrop: vi.fn() }));
import { onDragDrop, type DragDrop } from "../lib/tauri";

let emit: (e: DragDrop) => void = () => undefined;
const unlisten = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(onDragDrop).mockImplementation(async (h) => {
    emit = h;
    return unlisten;
  });
});

const LABEL = "Drop files or folders to match them to the manifest";
const setup = async (enabled = true, onDrop = vi.fn()) => {
  const utils = render(
    <DropZone enabled={enabled} disabledReason="Open a manifest first" label={LABEL} onDrop={onDrop} />,
  );
  await waitFor(() => expect(onDragDrop).toHaveBeenCalled());
  await act(async () => undefined);
  return { ...utils, onDrop };
};
const fire = (type: DragDrop["type"], paths: string[] = []) => act(() => emit({ type, paths }));

test("idle: renders nothing", async () => {
  const { container } = await setup();
  expect(container).toBeEmptyDOMElement();
});

test("hover shows the label on a plate; leave hides it", async () => {
  const { container } = await setup();
  fire("enter", ["C:\\a.dll"]);
  expect(screen.getByText(LABEL)).toBeInTheDocument();
  expect((await axe(container)).violations).toEqual([]);
  fire("over");
  expect(screen.getByText(LABEL)).toBeInTheDocument();
  fire("leave");
  expect(screen.queryByText(LABEL)).toBeNull();
});

test("drop hides the overlay and passes exactly the dropped paths", async () => {
  const { onDrop } = await setup();
  fire("enter", ["C:\\a.dll"]);
  fire("drop", ["C:\\a.dll", "C:\\dir"]);
  expect(screen.queryByText(LABEL)).toBeNull();
  expect(onDrop).toHaveBeenCalledWith(["C:\\a.dll", "C:\\dir"]);
});

test("disabled: overlay shows the reason and a drop is ignored", async () => {
  const { onDrop, container } = await setup(false);
  fire("enter", ["C:\\a.dll"]);
  expect(screen.getByText("Open a manifest first")).toBeInTheDocument();
  expect(screen.queryByText(LABEL)).toBeNull();
  expect((await axe(container)).violations).toEqual([]);
  fire("drop", ["C:\\a.dll"]);
  expect(onDrop).not.toHaveBeenCalled();
});

test("does not steal focus, and unsubscribes on unmount", async () => {
  const { unmount } = await setup();
  const before = document.activeElement;
  fire("enter");
  expect(document.activeElement).toBe(before);
  unmount();
  expect(unlisten).toHaveBeenCalled();
});
