import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { useAnnouncer } from "./useAnnouncer";

let frames: Map<number, FrameRequestCallback>;
let nextId: number;
let cancelled: number[];
const realRaf = globalThis.requestAnimationFrame;
const realCaf = globalThis.cancelAnimationFrame;

beforeEach(() => {
  frames = new Map();
  nextId = 1_000_000;
  cancelled = [];
  globalThis.requestAnimationFrame = (cb) => {
    const id = nextId++;
    frames.set(id, cb);
    return id;
  };
  globalThis.cancelAnimationFrame = (id) => {
    cancelled.push(id);
    frames.delete(id);
  };
});
afterEach(() => {
  globalThis.requestAnimationFrame = realRaf;
  globalThis.cancelAnimationFrame = realCaf;
  vi.restoreAllMocks();
});

const flushFrames = () =>
  act(() => {
    const due = [...frames.values()];
    frames.clear();
    due.forEach((cb) => cb(0));
  });

test("the first state is empty and announce is stable", () => {
  const { result, rerender } = renderHook(() => useAnnouncer());
  const first = result.current.announce;
  expect(result.current.text).toBe("");
  rerender();
  expect(result.current.announce).toBe(first);
});

test("announce commits the clear before the frame and the text after it", () => {
  const { result } = renderHook(() => useAnnouncer());
  act(() => result.current.announce("Saved."));
  expect(result.current.text).toBe("");
  expect(frames.size).toBe(1);
  flushFrames();
  expect(result.current.text).toBe("Saved.");
  // The same sentence again clears first, then returns.
  act(() => result.current.announce("Saved."));
  expect(result.current.text).toBe("");
  flushFrames();
  expect(result.current.text).toBe("Saved.");
});

test("a second announce before the frame replaces the pending text", () => {
  const { result } = renderHook(() => useAnnouncer());
  act(() => result.current.announce("first"));
  act(() => result.current.announce("second"));
  expect(frames.size).toBe(1);
  flushFrames();
  expect(result.current.text).toBe("second");
});

test("unmounting cancels the frame", () => {
  const { result, unmount } = renderHook(() => useAnnouncer());
  act(() => result.current.announce("bye"));
  const [id] = [...frames.keys()];
  unmount();
  expect(cancelled).toContain(id);
  expect(frames.size).toBe(0);
});
