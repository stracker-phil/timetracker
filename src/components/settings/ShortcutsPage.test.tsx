import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Keybindings, type Settings } from "../../api";
import { installCommands } from "../../commands";
import { KEYBINDINGS_KEY } from "../../queries/settings";
import { resetStore } from "../../state/store";
import { renderWithClient, seedQuery, SETTINGS_KEY } from "../../test/queries";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";
import { shortcutRows } from "./ShortcutsPage";

const mocked = vi.mocked(api, true);

function renderPage({ settings: saved = settings(), keybindings = {} }: { settings?: Settings; keybindings?: Keybindings } = {}) {
  resetStore();
  seedQuery(SETTINGS_KEY, saved);
  seedQuery(KEYBINDINGS_KEY, keybindings);
  installCommands();
  return renderWithClient(<SettingsWindow request={{ id: "shortcuts" }} onOpen={() => {}} />);
}

async function searchFor(text: string) {
  await userEvent.type(screen.getByRole("textbox", { name: "Search shortcuts" }), text);
}

const fields = () => screen.getAllByRole("button", { name: /Record shortcut|Press the new shortcut/ });
/** The search narrows the commands to one; the system-wide key's field comes first. */
const commandField = () => fields().at(-1)!;
const globalField = () => fields()[0];
const headings = () => screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent);

/** Records `keys` on the row labelled exactly `title`, found through the search. */
async function recordFor(title: string, keys: string) {
  await searchFor(title);
  const row = screen.getByText(title, { exact: true }).closest<HTMLElement>(".settings-row")!;
  await userEvent.click(within(row).getByRole("button", { name: "Record shortcut" }));
  await userEvent.keyboard(keys);
}

describe("Settings Shortcuts page", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("groups", () => {
    it.each(["System-wide", "In the main window", "In Day, Week, Month and Reports", "In the Day view", "With a suggestion focused", "In the Settings window"])("has the group %s", (title) => {
      renderPage();
      expect(screen.getByRole("heading", { name: title })).toBeInTheDocument();
    });

    it("orders the groups from system-wide to the Settings window", () => {
      renderPage();
      const order = ["System-wide", "In the main window", "In Day, Week, Month and Reports", "In the Day view", "With a suggestion focused", "In the Settings window"].map((title) => headings().indexOf(title));
      expect(order).toEqual([...order].sort((a, b) => a - b));
      expect(order).not.toContain(-1);
    });

    it("puts a command under the group of its scope", () => {
      renderPage();
      const group = screen.getByRole("region", { name: "In the Day view" });
      expect(within(group).getByText("Next activity")).toBeInTheDocument();
      expect(within(group).queryByText("Toggle sidebar")).not.toBeInTheDocument();
    });
  });

  describe("system-wide shortcut", () => {
    it("shows the saved timer key as Open timer picker", () => {
      renderPage();
      expect(screen.getByText("Open timer picker")).toBeInTheDocument();
      expect(globalField()).toHaveTextContent("⌘⇧T");
    });

    it("saves the recorded keys as the timer shortcut", async () => {
      renderPage();
      await userEvent.click(globalField());
      await userEvent.keyboard("{Meta>}{Shift>}y{/Shift}{/Meta}");
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ timerShortcut: "Mod+Shift+Y" }));
      await vi.waitFor(() => expect(globalField()).toHaveTextContent("⌘⇧Y"));
    });

    it("shows the refusal under the row and keeps the old shortcut", async () => {
      renderPage();
      mocked.updateSettings.mockRejectedValueOnce({ kind: "invalid_input", message: "macOS uses these keys", code: null });
      await userEvent.click(globalField());
      await userEvent.keyboard("{Meta>}{Shift>}y{/Shift}{/Meta}");
      expect(await screen.findByRole("alert")).toHaveTextContent("macOS uses these keys");
      expect(globalField()).toHaveTextContent("⌘⇧T");
    });

    it("turns the shortcut off and hides the remove button", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Remove shortcut for Open timer picker" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ timerShortcut: null }));
      await vi.waitFor(() => expect(globalField()).toHaveTextContent("unbound"));
      expect(screen.queryByRole("button", { name: "Remove shortcut for Open timer picker" })).not.toBeInTheDocument();
    });

    it("shows no remove button while unbound", () => {
      renderPage({ settings: settings({ timerShortcut: null }) });
      expect(screen.queryByRole("button", { name: "Remove shortcut for Open timer picker" })).not.toBeInTheDocument();
    });
  });

  describe("commands", () => {
    it("lists a command with its binding", async () => {
      renderPage();
      await searchFor("Toggle sidebar");
      expect(commandField()).toHaveTextContent("⌘⌃S");
    });

    it.each(["Launch at login", "Show welcome again"])("omits the menu-only command %s", (title) => {
      renderPage();
      expect(screen.getByText("Toggle sidebar")).toBeInTheDocument();
      expect(screen.queryByText(title)).not.toBeInTheDocument();
    });

    it("says so when no command matches the search", async () => {
      renderPage();
      await searchFor("nothing like this");
      expect(screen.getByText("No matching commands.")).toBeInTheDocument();
    });

    it("saves the recorded keys as the command's override", async () => {
      renderPage();
      await recordFor("Toggle sidebar", "{Meta>}{Shift>}j{/Shift}{/Meta}");
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": "Mod+Shift+J" });
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("⌘⇧J"));
    });

    it("unbinds the command when Backspace is pressed while recording", async () => {
      renderPage();
      await recordFor("Toggle sidebar", "{Backspace}");
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": null });
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("unbound"));
    });

    it("keeps the binding when Escape cancels the recording", async () => {
      renderPage();
      await recordFor("Toggle sidebar", "{Escape}");
      expect(mocked.setKeybindings).not.toHaveBeenCalled();
      expect(commandField()).toHaveTextContent("⌘⌃S");
    });

    describe("commands Rust runs", () => {
      it("lists a bus command with its default binding", async () => {
        renderPage();
        await searchFor("Hide to Menu Bar");
        expect(commandField()).toHaveTextContent("⌘Q");
      });

      it("finds a bus command by its id", async () => {
        renderPage();
        await searchFor("app:quit");
        expect(screen.getByText("Quit TimeTracker")).toBeInTheDocument();
        expect(commandField()).toHaveTextContent("unbound");
      });

      it("saves a rebinding under the bus command's id", async () => {
        renderPage();
        await recordFor("Hide to Menu Bar", "{Meta>}{Shift>}h{/Shift}{/Meta}");
        expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "app:background": "Mod+Shift+H" });
        await vi.waitFor(() => expect(commandField()).toHaveTextContent("⌘⇧H"));
      });
    });
  });

  describe("removing a shortcut", () => {
    it("saves a null override and hides the remove button", async () => {
      renderPage();
      await searchFor("Toggle sidebar");
      await userEvent.click(screen.getByRole("button", { name: "Remove shortcut for Toggle sidebar" }));
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": null });
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("unbound"));
      expect(screen.queryByRole("button", { name: "Remove shortcut for Toggle sidebar" })).not.toBeInTheDocument();
    });

    it("shows no remove button for a command without a key", async () => {
      renderPage();
      await searchFor("app:quit");
      expect(screen.queryByRole("button", { name: "Remove shortcut for Quit TimeTracker" })).not.toBeInTheDocument();
    });
  });

  describe("resetting a shortcut", () => {
    it("shows no reset button while the key is the default", async () => {
      renderPage();
      await searchFor("Toggle sidebar");
      expect(screen.queryByRole("button", { name: "Reset Toggle sidebar" })).not.toBeInTheDocument();
    });

    it("shows the reset button for a changed key and removes the override with it", async () => {
      renderPage({ keybindings: { "sidebar.toggle": "Mod+Shift+J" } });
      await searchFor("Toggle sidebar");
      expect(commandField()).toHaveTextContent("⌘⇧J");
      await userEvent.click(screen.getByRole("button", { name: "Reset Toggle sidebar" }));
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({});
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("⌘⌃S"));
      expect(screen.queryByRole("button", { name: "Reset Toggle sidebar" })).not.toBeInTheDocument();
    });

    it("shows the reset button for a command whose default key was removed", async () => {
      renderPage({ keybindings: { "sidebar.toggle": null } });
      await searchFor("Toggle sidebar");
      expect(screen.getByRole("button", { name: "Reset Toggle sidebar" })).toBeInTheDocument();
    });

    it("asks before Reset takes a default key another command holds now", async () => {
      renderPage({ keybindings: { "sidebar.toggle": "Mod+Shift+J", "view.week": "Mod+Ctrl+S" } });
      await searchFor("Toggle sidebar");
      await userEvent.click(screen.getByRole("button", { name: "Reset Toggle sidebar" }));
      expect(screen.getByRole("dialog", { name: "⌘⌃S is used by “Show Week”" })).toBeInTheDocument();
      expect(mocked.setKeybindings).not.toHaveBeenCalled();
    });

    it("drops the override when the default key is recorded again", async () => {
      renderPage({ keybindings: { "sidebar.toggle": "Mod+Shift+J", "view.week": "Mod+Shift+W" } });
      await recordFor("Toggle sidebar", "{Meta>}{Control>}s{/Control}{/Meta}");
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "view.week": "Mod+Shift+W" });
    });
  });

  describe("a key another command holds", () => {
    it("asks before taking it and names both commands", async () => {
      renderPage();
      await recordFor("Toggle sidebar", "{Meta>}k{/Meta}");
      const sheet = screen.getByRole("dialog", { name: "⌘K is used by “Command palette”" });
      expect(sheet).toHaveTextContent("Reassign it to “Toggle sidebar”?");
      expect(mocked.setKeybindings).not.toHaveBeenCalled();
    });

    it("moves the key to the command and unbinds the holder on Reassign", async () => {
      renderPage();
      await recordFor("Toggle sidebar", "{Meta>}k{/Meta}");
      await userEvent.click(screen.getByRole("button", { name: "Reassign" }));
      expect(mocked.setKeybindings).toHaveBeenCalledWith({ "sidebar.toggle": "Mod+K", "palette.toggle": null });
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("⌘K"));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("saves nothing on Cancel and keeps the old key", async () => {
      renderPage();
      await recordFor("Toggle sidebar", "{Meta>}k{/Meta}");
      await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mocked.setKeybindings).not.toHaveBeenCalled();
      expect(commandField()).toHaveTextContent("⌘⌃S");
    });

    it("saves nothing when Escape closes the sheet", async () => {
      renderPage();
      await recordFor("Toggle sidebar", "{Meta>}k{/Meta}");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mocked.setKeybindings).not.toHaveBeenCalled();
    });

    it("binds a free key without asking", async () => {
      renderPage();
      await recordFor("Next activity", "{Meta>}{Shift>}j{/Shift}{/Meta}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "day.nextBlock": "Mod+Shift+J" });
    });

    it("shows no conflict warning for two commands that share a key", () => {
      renderPage({ keybindings: { "view.week": "Mod+1" } });
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    });
  });

  describe("shortcutRows", () => {
    it("lists Open timer picker and one row per command that can have a key", () => {
      installCommands();
      const rows = shortcutRows();
      expect(rows[0]).toMatchObject({ id: "global-shortcut", title: "Open timer picker" });
      expect(rows).toContainEqual({ id: "shortcut:sidebar.toggle", title: "Toggle sidebar" });
      expect(rows).toContainEqual({ id: "shortcut:app:quit", title: "Quit TimeTracker" });
    });

    it.each(["welcome.show", "app:launchAtLogin"])("leaves out the menu-only command %s", (id) => {
      installCommands();
      expect(shortcutRows().map((r) => r.id)).not.toContain(`shortcut:${id}`);
    });
  });
});
