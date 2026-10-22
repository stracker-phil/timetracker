import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../../api";
import { installCommands } from "../../commands";
import { resetStore, useStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsDialog } from "./SettingsDialog";

const mocked = vi.mocked(api, true);

function openPage(name: string) {
  return userEvent.click(within(screen.getByRole("navigation", { name: "Settings pages" })).getByRole("button", { name }));
}

function renderDialog(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore({ settings: settings(), dialog: { type: "settings" }, ...patch });
  installCommands();
  return render(<SettingsDialog />);
}

describe("SettingsDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("opens on the Appearance page", () => {
    renderDialog();
    expect(screen.getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
  });

  it("switches the shown page from the navigation", async () => {
    renderDialog();
    await openPage("Logging");
    expect(screen.getByRole("heading", { name: "Logging" })).toBeInTheDocument();
  });

  describe("Appearance", () => {
    it.each([
      ["Light", "light"],
      ["Dark", "dark"],
    ])("saves the %s scheme of the clicked tile", async (name, scheme) => {
      renderDialog();
      await userEvent.click(within(screen.getByRole("group", { name: "Scheme" })).getByRole("button", { name }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ scheme }));
    });

    it("marks the current scheme tile as pressed", () => {
      renderDialog({ settings: settings({ scheme: "dark" }) });
      const group = within(screen.getByRole("group", { name: "Scheme" }));
      expect(group.getByRole("button", { name: "Dark" })).toHaveAttribute("aria-pressed", "true");
      expect(group.getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "false");
    });

    it("saves the accent of the clicked swatch", async () => {
      renderDialog();
      await userEvent.click(screen.getByRole("button", { name: "Accent teal" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accent: "teal" }));
    });

    it("marks the current accent as pressed", () => {
      renderDialog();
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Accent teal" })).toHaveAttribute("aria-pressed", "false");
    });

    it.each([
      ["Light", "light"],
      ["Dark", "dark"],
      ["System", "system"],
    ])("saves the %s mode", async (name, mode) => {
      renderDialog({ settings: settings({ mode: mode === "light" ? "dark" : "light" }) });
      await userEvent.click(within(screen.getByRole("group", { name: "Mode" })).getByRole("button", { name }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ mode }));
    });

    it("saves a typed text size on blur, not per keystroke", async () => {
      renderDialog();
      const size = screen.getByLabelText("UI size");
      await userEvent.clear(size);
      await userEvent.type(size, "16");
      expect(mocked.updateSettings).not.toHaveBeenCalled();
      await userEvent.tab();
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ uiFontSize: 16 }));
    });

    it("restores the saved size instead of saving an out-of-range one", async () => {
      renderDialog();
      const size = screen.getByLabelText("UI size");
      await userEvent.clear(size);
      await userEvent.type(size, "99");
      await userEvent.tab();
      expect(mocked.updateSettings).not.toHaveBeenCalled();
      expect(size).toHaveValue(13);
    });

    it("saves an empty font name as null", async () => {
      renderDialog({ settings: settings({ fontUi: "Inter" }) });
      const font = screen.getByLabelText("UI font");
      expect(font).toHaveValue("Inter");
      await userEvent.clear(font);
      await userEvent.tab();
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ fontUi: null }));
    });
  });

  describe("System", () => {
    it("lists General first among the System pages", () => {
      renderDialog();
      const pages = within(screen.getByRole("navigation", { name: "Settings pages" })).getAllByRole("button");
      const names = pages.map((p) => p.textContent?.trim());
      expect(names.indexOf("General")).toBeLessThan(names.indexOf("Data"));
      expect(names.indexOf("Data")).toBeLessThan(names.indexOf("Logging"));
    });

    it("installs launch at login when the switch is turned on", async () => {
      renderDialog();
      await openPage("General");
      await userEvent.click(screen.getByRole("switch", { name: "Launch at login" }));
      expect(mocked.autostartSet).toHaveBeenLastCalledWith(true);
      await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Launch at login" })).toBeChecked());
    });

    it("shows the launch at login switch on when the LaunchAgent is installed", async () => {
      mocked.autostartStatus.mockResolvedValueOnce(true);
      renderDialog();
      await openPage("General");
      await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Launch at login" })).toBeChecked());
    });

    it("closes Settings and shows the welcome sheet again", async () => {
      renderDialog({ uiState: { ...EMPTY_UI_STATE, onboardingDone: true } });
      await openPage("General");
      await userEvent.click(screen.getByRole("button", { name: "Show welcome again" }));
      expect(useStore.getState().dialog).toBeNull();
      expect(useStore.getState().uiState.onboardingDone).toBe(false);
    });

    it("saves the log level chosen in the select", async () => {
      renderDialog();
      await openPage("Logging");
      await userEvent.selectOptions(screen.getByRole("combobox", { name: "Log level" }), "Debug");
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ logLevel: "debug" }));
    });

    it("opens the log folder", async () => {
      renderDialog();
      await openPage("Logging");
      await userEvent.click(screen.getByRole("button", { name: /Open log folder/ }));
      expect(mocked.openLogFolder).toHaveBeenCalled();
    });
  });

  describe("Shortcuts", () => {
    async function openShortcuts() {
      await openPage("Shortcuts");
      await userEvent.type(screen.getByRole("textbox", { name: "Search shortcuts" }), "Toggle sidebar");
    }

    const fields = () => screen.getAllByRole("button", { name: /Record shortcut|Press the new shortcut/ });
    /** The search narrows the commands to one; the global shortcut's field comes first. */
    const commandField = () => fields().at(-1)!;
    const globalField = () => fields()[0];

    describe("global shortcut", () => {
      it("shows the saved start-timer shortcut under its group", async () => {
        renderDialog();
        await openPage("Shortcuts");
        expect(screen.getByText("Global shortcut")).toBeInTheDocument();
        expect(screen.getByText("Start timer (system-wide)")).toBeInTheDocument();
        expect(globalField()).toHaveTextContent("⌘⇧T");
      });

      it("saves the recorded keys as the timer shortcut", async () => {
        renderDialog();
        await openPage("Shortcuts");
        await userEvent.click(globalField());
        await userEvent.keyboard("{Meta>}{Shift>}y{/Shift}{/Meta}");
        expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ timerShortcut: "Mod+Shift+Y" }));
        await vi.waitFor(() => expect(globalField()).toHaveTextContent("⌘⇧Y"));
      });

      it("shows the refusal under the row and keeps the old shortcut", async () => {
        renderDialog();
        await openPage("Shortcuts");
        mocked.updateSettings.mockRejectedValueOnce({ kind: "invalid_input", message: "macOS uses these keys", code: null });
        await userEvent.click(globalField());
        await userEvent.keyboard("{Meta>}{Shift>}y{/Shift}{/Meta}");
        expect(await screen.findByRole("alert")).toHaveTextContent("macOS uses these keys");
        expect(globalField()).toHaveTextContent("⌘⇧T");
      });
    });

    it("groups the in-app commands under In TimeTracker", async () => {
      renderDialog();
      await openPage("Shortcuts");
      expect(screen.getByText("In TimeTracker")).toBeInTheDocument();
    });

    it("lists the commands with their bindings", async () => {
      renderDialog();
      await openShortcuts();
      expect(commandField()).toHaveTextContent("⌘⌃S");
    });

    it("says so when no command matches the search", async () => {
      renderDialog();
      await openPage("Shortcuts");
      await userEvent.type(screen.getByRole("textbox", { name: "Search shortcuts" }), "nothing like this");
      expect(screen.getByText("No matching commands.")).toBeInTheDocument();
    });

    it("saves the keys pressed while recording as the command's binding", async () => {
      renderDialog();
      await openShortcuts();
      await userEvent.click(commandField());
      await userEvent.keyboard("{Meta>}{Shift>}j{/Shift}{/Meta}");
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": "Mod+Shift+J" });
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("⌘⇧J"));
    });

    it("unbinds the command when Backspace is pressed while recording", async () => {
      renderDialog();
      await openShortcuts();
      await userEvent.click(commandField());
      await userEvent.keyboard("{Backspace}");
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": null });
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("unbound"));
    });

    it("keeps the binding when Escape cancels the recording", async () => {
      renderDialog();
      await openShortcuts();
      await userEvent.click(commandField());
      await userEvent.keyboard("{Escape}");
      expect(mocked.setKeybindings).not.toHaveBeenCalled();
      expect(commandField()).toHaveTextContent("⌘⌃S");
    });

    it("removes the override when the reset button is clicked", async () => {
      renderDialog({ keybindings: { "sidebar.toggle": "Mod+Shift+J" } });
      await openShortcuts();
      expect(commandField()).toHaveTextContent("⌘⇧J");
      await userEvent.click(screen.getByRole("button", { name: "Reset Toggle sidebar" }));
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({});
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("⌘⌃S"));
    });

    it("warns when two commands share a binding", async () => {
      renderDialog({ keybindings: { "view.week": "Mod+1" } });
      await openPage("Shortcuts");
      expect(screen.getByRole("alert")).toHaveTextContent("⌘1 is used by both “Show Day” and “Show Week”.");
    });
  });
});
