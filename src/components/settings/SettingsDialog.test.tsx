import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const system = vi.hoisted(() => {
  const state = { dark: false };
  window.matchMedia = ((query: string) => ({
    get matches() {
      return state.dark;
    },
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }));
  return state;
});

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
    const modeGroup = () => within(screen.getByRole("group", { name: "Mode" }));

    afterEach(() => {
      system.dark = false;
    });

    it("explains that a scheme is picked for each macOS mode", () => {
      renderDialog();
      expect(screen.getByText("Pick a scheme for each macOS mode.")).toBeInTheDocument();
    });

    it("offers Light and Dark modes only", () => {
      renderDialog();
      expect(modeGroup().getAllByRole("button").map((b) => b.textContent)).toEqual(["Light", "Dark"]);
    });

    it("starts on Light and its scheme picker while macOS is light", () => {
      renderDialog();
      expect(modeGroup().getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "true");
      expect(modeGroup().getByRole("button", { name: "Dark" })).toHaveAttribute("aria-pressed", "false");
      expect(screen.getByRole("group", { name: "Light mode scheme" })).toBeInTheDocument();
      expect(screen.queryByRole("group", { name: "Dark mode scheme" })).not.toBeInTheDocument();
    });

    it("starts on Dark and its scheme picker while macOS is dark", () => {
      system.dark = true;
      renderDialog();
      expect(modeGroup().getByRole("button", { name: "Dark" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("group", { name: "Dark mode scheme" })).toBeInTheDocument();
    });

    it("switches the picker and its pressed tile when the mode tab changes", async () => {
      renderDialog({ settings: settings({ lightScheme: "light", darkScheme: "default" }) });
      expect(within(screen.getByRole("group", { name: "Light mode scheme" })).getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "true");
      await userEvent.click(modeGroup().getByRole("button", { name: "Dark" }));
      const dark = within(screen.getByRole("group", { name: "Dark mode scheme" }));
      expect(screen.queryByRole("group", { name: "Light mode scheme" })).not.toBeInTheDocument();
      expect(dark.getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "true");
      expect(dark.getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "false");
    });

    it("saves the clicked tile as the light scheme and leaves the dark one unchanged", async () => {
      renderDialog({ settings: settings({ lightScheme: "default", darkScheme: "dark" }) });
      await userEvent.click(within(screen.getByRole("group", { name: "Light mode scheme" })).getByRole("button", { name: "Dark" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ lightScheme: "dark", darkScheme: "dark" }));
    });

    it("saves the clicked tile as the dark scheme and leaves the light one unchanged", async () => {
      renderDialog({ settings: settings({ lightScheme: "default", darkScheme: "dark" }) });
      await userEvent.click(modeGroup().getByRole("button", { name: "Dark" }));
      await userEvent.click(within(screen.getByRole("group", { name: "Dark mode scheme" })).getByRole("button", { name: "Light" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ lightScheme: "default", darkScheme: "light" }));
    });

    it("does not save anything when only the mode tab changes", async () => {
      renderDialog();
      await userEvent.click(modeGroup().getByRole("button", { name: "Dark" }));
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it("saves the accent of the clicked swatch", async () => {
      renderDialog();
      await userEvent.click(screen.getByRole("button", { name: "Accent teal" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accent: "teal" }));
    });

    it("saves the system accent when the System swatch is clicked", async () => {
      renderDialog();
      await userEvent.click(screen.getByRole("button", { name: "Accent system" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accent: "system" }));
    });

    it("marks the System swatch as pressed while the accent is system", () => {
      renderDialog({ settings: settings({ accent: "system" }) });
      expect(screen.getByRole("button", { name: "Accent system" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "false");
    });

    it("marks the current accent as pressed", () => {
      renderDialog();
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Accent teal" })).toHaveAttribute("aria-pressed", "false");
    });

    describe("Dates", () => {
      const control = (name: string) => within(screen.getByRole("group", { name }));

      it.each([
        ["Calendar title", ["Short", "Full"]],
        ["Day title", ["Long", "Short"]],
      ])("offers %s as %j", (name, options) => {
        renderDialog();
        expect(control(name).getAllByRole("button").map((b) => b.textContent)).toEqual(options);
      });

      it("presses the saved choices", () => {
        renderDialog({ settings: settings({ calendarTitle: "full", dayTitle: "short" }) });
        expect(control("Calendar title").getByRole("button", { name: "Full" })).toHaveAttribute("aria-pressed", "true");
        expect(control("Calendar title").getByRole("button", { name: "Short" })).toHaveAttribute("aria-pressed", "false");
        expect(control("Day title").getByRole("button", { name: "Short" })).toHaveAttribute("aria-pressed", "true");
        expect(control("Day title").getByRole("button", { name: "Long" })).toHaveAttribute("aria-pressed", "false");
      });

      it("saves calendarTitle when the Calendar title is changed", async () => {
        renderDialog();
        await userEvent.click(control("Calendar title").getByRole("button", { name: "Full" }));
        expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ calendarTitle: "full", dayTitle: "long" }));
        await vi.waitFor(() => expect(control("Calendar title").getByRole("button", { name: "Full" })).toHaveAttribute("aria-pressed", "true"));
      });

      it("saves dayTitle when the Day title is changed", async () => {
        renderDialog();
        await userEvent.click(control("Day title").getByRole("button", { name: "Short" }));
        expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ dayTitle: "short", calendarTitle: "short" }));
        await vi.waitFor(() => expect(control("Day title").getByRole("button", { name: "Short" })).toHaveAttribute("aria-pressed", "true"));
      });
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
