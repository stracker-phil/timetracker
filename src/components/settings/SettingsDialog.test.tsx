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

  it("opens on General, the first page", () => {
    renderDialog();
    expect(screen.getByRole("heading", { name: "General" })).toBeInTheDocument();
  });

  it("switches the shown page from the navigation", async () => {
    renderDialog();
    await openPage("Appearance");
    expect(screen.getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
  });

  describe("navigation", () => {
    const nav = () => screen.getByRole("navigation", { name: "Settings pages" });

    it("lists the pages in order", () => {
      renderDialog();
      expect(within(nav()).getAllByRole("button").map((b) => b.textContent?.trim())).toEqual(["General", "Appearance", "Shortcuts", "Capture", "Tracker", "Timeline", "Data"]);
    });

    it("groups them under App, Tracking and System", () => {
      renderDialog();
      expect(nav().textContent).toMatch(/^\s*App\s*General\s*Appearance\s*Shortcuts\s*Tracking\s*Capture\s*Tracker\s*Timeline\s*System\s*Data\s*$/);
    });

    it("has no Logging page", () => {
      renderDialog();
      expect(within(nav()).queryByRole("button", { name: "Logging" })).not.toBeInTheDocument();
    });
  });

  describe("Appearance", () => {
    const schemes = (name: string) => within(screen.getByRole("group", { name }));
    const renderAppearance = (patch: Parameters<typeof settings>[0] = {}) =>
      renderDialog({ settings: settings(patch), dialog: { type: "settings", page: "appearance" } });

    it("has no Mode switcher", () => {
      renderAppearance();
      expect(screen.queryByRole("group", { name: "Mode" })).not.toBeInTheDocument();
    });

    it.each(["Light mode scheme", "Dark mode scheme"])("offers %s as Default, Light and Dark", (name) => {
      renderAppearance();
      expect(schemes(name).getAllByRole("button").map((b) => b.textContent)).toEqual(["Default", "Light", "Dark"]);
    });

    it("presses the saved scheme in each group", () => {
      renderAppearance({ lightScheme: "light", darkScheme: "default" });
      expect(schemes("Light mode scheme").getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "true");
      expect(schemes("Light mode scheme").getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "false");
      expect(schemes("Dark mode scheme").getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "true");
      expect(schemes("Dark mode scheme").getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "false");
    });

    it("saves the clicked tile as the light scheme and leaves the dark one unchanged", async () => {
      renderAppearance({ lightScheme: "default", darkScheme: "dark" });
      await userEvent.click(schemes("Light mode scheme").getByRole("button", { name: "Dark" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ lightScheme: "dark", darkScheme: "dark" }));
    });

    it("saves the clicked tile as the dark scheme and leaves the light one unchanged", async () => {
      renderAppearance({ lightScheme: "default", darkScheme: "dark" });
      await userEvent.click(schemes("Dark mode scheme").getByRole("button", { name: "Light" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ lightScheme: "default", darkScheme: "light" }));
    });

    describe("Text size", () => {
      const sizes = () => schemes("Text size");

      it("offers Small, Medium and Large", () => {
        renderAppearance();
        expect(sizes().getAllByRole("button").map((b) => b.textContent)).toEqual(["Small", "Medium", "Large"]);
      });

      it("presses the saved size", () => {
        renderAppearance({ textSize: "large" });
        expect(sizes().getByRole("button", { name: "Large" })).toHaveAttribute("aria-pressed", "true");
        expect(sizes().getByRole("button", { name: "Medium" })).toHaveAttribute("aria-pressed", "false");
      });

      it("saves the clicked size", async () => {
        renderAppearance();
        await userEvent.click(sizes().getByRole("button", { name: "Small" }));
        expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ textSize: "small" }));
      });

      it("shows no pixel sizes", () => {
        renderAppearance();
        expect(document.body.textContent).not.toMatch(/\d\s*px|\bpx\b/i);
      });

      it.each(["UI size", "Data size", "UI font", "Mono font"])("has no %s field", (label) => {
        renderAppearance();
        expect(screen.queryByLabelText(label)).not.toBeInTheDocument();
      });
    });

    it("saves the accent of the clicked swatch", async () => {
      renderAppearance();
      await userEvent.click(screen.getByRole("button", { name: "Accent teal" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accent: "teal" }));
    });

    it("saves the system accent when the System swatch is clicked", async () => {
      renderAppearance();
      await userEvent.click(screen.getByRole("button", { name: "Accent system" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accent: "system" }));
    });

    it("marks the System swatch as pressed while the accent is system", () => {
      renderAppearance({ accent: "system" });
      expect(screen.getByRole("button", { name: "Accent system" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "false");
    });

    it("marks the current accent as pressed", () => {
      renderAppearance();
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Accent teal" })).toHaveAttribute("aria-pressed", "false");
    });

    describe("Dates", () => {
      const control = (name: string) => within(screen.getByRole("group", { name }));

      it.each([
        ["Calendar title", ["Short", "Full"]],
        ["Day title", ["Long", "Short"]],
      ])("offers %s as %j", (name, options) => {
        renderAppearance();
        expect(control(name).getAllByRole("button").map((b) => b.textContent)).toEqual(options);
      });

      it("presses the saved choices", () => {
        renderAppearance({ calendarTitle: "full", dayTitle: "short" });
        expect(control("Calendar title").getByRole("button", { name: "Full" })).toHaveAttribute("aria-pressed", "true");
        expect(control("Calendar title").getByRole("button", { name: "Short" })).toHaveAttribute("aria-pressed", "false");
        expect(control("Day title").getByRole("button", { name: "Short" })).toHaveAttribute("aria-pressed", "true");
        expect(control("Day title").getByRole("button", { name: "Long" })).toHaveAttribute("aria-pressed", "false");
      });

      it("saves calendarTitle when the Calendar title is changed", async () => {
        renderAppearance();
        await userEvent.click(control("Calendar title").getByRole("button", { name: "Full" }));
        expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ calendarTitle: "full", dayTitle: "long" }));
        await vi.waitFor(() => expect(control("Calendar title").getByRole("button", { name: "Full" })).toHaveAttribute("aria-pressed", "true"));
      });

      it("saves dayTitle when the Day title is changed", async () => {
        renderAppearance();
        await userEvent.click(control("Day title").getByRole("button", { name: "Short" }));
        expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ dayTitle: "short", calendarTitle: "short" }));
        await vi.waitFor(() => expect(control("Day title").getByRole("button", { name: "Short" })).toHaveAttribute("aria-pressed", "true"));
      });
    });

  });

  describe("General", () => {
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

    it.each(["Launch at login", "Show welcome again"])("omits the menu-only command %s", async (title) => {
      renderDialog();
      await openPage("Shortcuts");
      expect(screen.getByText("Toggle sidebar")).toBeInTheDocument();
      expect(screen.queryByText(title)).not.toBeInTheDocument();
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
