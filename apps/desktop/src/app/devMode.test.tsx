import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { BuildResult } from "../tools/tarpack/BuildResult";
import { ManifestHeader } from "../tools/tarpack/ManifestHeader";
import { ShortcutsHelp } from "../tools/tarpack/ShortcutsHelp";
import { entry, manifest, session, summary } from "../tools/tarpack/fixtures";
import { DevModeProvider, useDevMode } from "./devMode";

vi.mock("../lib/tarpack", () => ({ recentManifests: vi.fn().mockResolvedValue([]) }));

function Probe() {
  return <p>{useDevMode() ? "dev on" : "dev off"}</p>;
}

test("Ctrl+Alt+Shift+D toggles developer mode and announces it", async () => {
  const user = userEvent.setup();
  render(
    <DevModeProvider>
      <Probe />
    </DevModeProvider>,
  );
  expect(screen.getByText("dev off")).toBeInTheDocument();
  await user.keyboard("{Control>}{Alt>}{Shift>}D{/Shift}{/Alt}{/Control}");
  expect(screen.getByText("dev on")).toBeInTheDocument();
  expect(screen.getByTestId("dev-mode-announcer")).toHaveTextContent("Developer mode on.");
  await user.keyboard("{Control>}{Alt>}{Shift>}D{/Shift}{/Alt}{/Control}");
  expect(screen.getByText("dev off")).toBeInTheDocument();
  expect(screen.getByTestId("dev-mode-announcer")).toHaveTextContent("Developer mode off.");
});

test("other chords leave developer mode off", async () => {
  const user = userEvent.setup();
  render(
    <DevModeProvider>
      <Probe />
    </DevModeProvider>,
  );
  await user.keyboard("{Control>}{Shift>}D{/Shift}{/Control}");
  await user.keyboard("{Control>}{Alt>}D{/Alt}{/Control}");
  expect(screen.getByText("dev off")).toBeInTheDocument();
});

test("without developer mode, Edit in editor and the extraction command are hidden", async () => {
  const h = { onOpen: vi.fn(), onOpenRecent: vi.fn(), onReload: vi.fn(), onEdit: vi.fn() };
  render(
    <DevModeProvider>
      <ManifestHeader session={session(manifest())} {...h} />
      <BuildResult
        result={{ ok: summary() }}
        entries={[entry("gateway")]}
        onReveal={() => undefined}
        onDismiss={() => undefined}
      />
    </DevModeProvider>,
  );
  await screen.findByRole("button", { name: "Reload" });
  expect(screen.queryByRole("button", { name: "Edit in editor" })).toBeNull();
  expect(screen.queryByText("Extract on the target")).toBeNull();
  expect(screen.queryByRole("button", { name: "Copy extraction command" })).toBeNull();

  act(() => {
    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "D", code: "KeyD", ctrlKey: true, altKey: true, shiftKey: true }),
    );
  });
  expect(screen.getByRole("button", { name: "Edit in editor" })).toBeInTheDocument();
  expect(screen.getByText("Extract on the target")).toBeInTheDocument();
});

test("the shortcuts list names Ctrl+E only in developer mode", () => {
  const { container } = render(
    <DevModeProvider>
      <ShortcutsHelp />
    </DevModeProvider>,
  );
  expect(container).toHaveTextContent("Reload manifest");
  expect(container).not.toHaveTextContent("Edit in editor");
});
