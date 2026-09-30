import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { beforeEach, expect, test, vi } from "vitest";
import { ManifestHeader } from "./ManifestHeader";
import { manifest, session } from "./fixtures";

vi.mock("../../lib/tarpack", () => ({ recentManifests: vi.fn() }));
import { recentManifests } from "../../lib/tarpack";

const handlers = () => ({ onOpen: vi.fn(), onOpenRecent: vi.fn(), onReload: vi.fn(), onEdit: vi.fn() });

beforeEach(() => {
  vi.mocked(recentManifests).mockResolvedValue(["C:\\a\\one.toml", "C:\\b\\two.toml"]);
});

test("each action calls its handler", async () => {
  const user = userEvent.setup();
  const h = handlers();
  render(<ManifestHeader session={session(manifest({ errors: [] }))} {...h} />);
  await screen.findByRole("button", { name: "Recent" });
  await user.click(screen.getByRole("button", { name: "Open…" }));
  await user.click(screen.getByRole("button", { name: "Reload" }));
  await user.click(screen.getByRole("button", { name: "Edit in editor" }));
  expect(h.onOpen).toHaveBeenCalled();
  expect(h.onReload).toHaveBeenCalled();
  expect(h.onEdit).toHaveBeenCalled();
  expect(screen.getByRole("heading", { level: 1, name: "gateway" })).toBeInTheDocument();
  expect(screen.getByTitle("C:\\pkgs\\gateway\\manifest.toml")).toBeInTheDocument();
});

test("Recent is hidden when empty", async () => {
  vi.mocked(recentManifests).mockResolvedValue([]);
  render(<ManifestHeader session={session(manifest())} {...handlers()} />);
  await screen.findByRole("button", { name: "Reload" });
  expect(screen.queryByRole("button", { name: "Recent" })).toBeNull();
});

test("Recent menu opens, moves, and closes with Escape returning focus", async () => {
  const user = userEvent.setup();
  const h = handlers();
  const { container } = render(<ManifestHeader session={session(manifest())} {...h} />);
  const trigger = await screen.findByRole("button", { name: "Recent" });
  trigger.focus();
  await user.keyboard("{Enter}");
  const items = screen.getAllByRole("menuitem");
  expect(items).toHaveLength(2);
  expect(items[0]).toHaveFocus();
  expect(trigger).toHaveAttribute("aria-expanded", "true");
  await user.keyboard("{ArrowDown}");
  expect(items[1]).toHaveFocus();
  expect((await axe(container)).violations).toEqual([]);
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("menu")).toBeNull();
  expect(trigger).toHaveFocus();
  await user.keyboard("{ArrowDown}");
  expect(screen.getAllByRole("menuitem")[0]).toHaveFocus();
  await user.keyboard("{Enter}");
  expect(h.onOpenRecent).toHaveBeenCalledWith("C:\\a\\one.toml");
});

test("Tab closes the menu and focus is not lost to body", async () => {
  const user = userEvent.setup();
  render(<ManifestHeader session={session(manifest())} {...handlers()} />);
  const trigger = await screen.findByRole("button", { name: "Recent" });
  trigger.focus();
  await user.keyboard("{Enter}");
  expect(screen.getAllByRole("menuitem")[0]).toHaveFocus();
  expect(screen.getAllByRole("menuitem")[0]).toHaveAttribute("tabindex", "-1");
  await user.keyboard("{Tab}");
  expect(screen.queryByRole("menu")).toBeNull();
  expect(document.activeElement).not.toBe(document.body);
});
