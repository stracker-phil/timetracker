import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { installCommands } from "../../commands";
import { resetStore } from "../../state/store";
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
    it("saves the dark scheme chosen in the select", async () => {
      renderDialog();
      await userEvent.selectOptions(screen.getByLabelText("Dark scheme"), "Nord");
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ darkScheme: "nord" }));
    });

    it("saves the light scheme chosen in the select", async () => {
      renderDialog();
      await userEvent.selectOptions(screen.getByLabelText("Light scheme"), "GitHub Light");
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ lightScheme: "github-light" }));
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
    ])("saves the %s theme", async (name, theme) => {
      renderDialog({ settings: settings({ theme: theme === "light" ? "dark" : "light" }) });
      await userEvent.click(within(screen.getByRole("group", { name: "Theme" })).getByRole("button", { name }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ theme }));
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
    it("saves launch at login when the switch is turned on", async () => {
      renderDialog();
      await openPage("General");
      await userEvent.click(screen.getByRole("switch", { name: "Launch at login" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ launchAtLogin: true }));
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

    it("lists the commands with their bindings", async () => {
      renderDialog();
      await openShortcuts();
      expect(screen.getByRole("button", { name: "Record shortcut" })).toHaveTextContent("⌘⌃S");
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
      await userEvent.click(screen.getByRole("button", { name: "Record shortcut" }));
      await userEvent.keyboard("{Meta>}{Shift>}j{/Shift}{/Meta}");
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": "Mod+Shift+J" });
      expect(await screen.findByRole("button", { name: "Record shortcut" })).toHaveTextContent("⌘⇧J");
    });

    it("unbinds the command when Backspace is pressed while recording", async () => {
      renderDialog();
      await openShortcuts();
      await userEvent.click(screen.getByRole("button", { name: "Record shortcut" }));
      await userEvent.keyboard("{Backspace}");
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": null });
      expect(await screen.findByRole("button", { name: "Record shortcut" })).toHaveTextContent("unbound");
    });

    it("keeps the binding when Escape cancels the recording", async () => {
      renderDialog();
      await openShortcuts();
      await userEvent.click(screen.getByRole("button", { name: "Record shortcut" }));
      await userEvent.keyboard("{Escape}");
      expect(mocked.setKeybindings).not.toHaveBeenCalled();
      expect(screen.getByRole("button", { name: "Record shortcut" })).toHaveTextContent("⌘⌃S");
    });

    it("removes the override when the reset button is clicked", async () => {
      renderDialog({ keybindings: { "sidebar.toggle": "Mod+Shift+J" } });
      await openShortcuts();
      expect(screen.getByRole("button", { name: "Record shortcut" })).toHaveTextContent("⌘⇧J");
      await userEvent.click(screen.getByRole("button", { name: "Reset Toggle sidebar" }));
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({});
      expect(await screen.findByRole("button", { name: "Record shortcut" })).toHaveTextContent("⌘⌃S");
    });

    it("warns when two commands share a binding", async () => {
      renderDialog({ keybindings: { "view.week": "Mod+1" } });
      await openPage("Shortcuts");
      expect(screen.getByRole("alert")).toHaveTextContent("⌘1 is used by both “Show Day” and “Show Week”.");
    });
  });
});
