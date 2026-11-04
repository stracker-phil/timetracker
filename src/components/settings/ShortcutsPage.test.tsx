import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { installCommands } from "../../commands";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

function renderPage(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore({ settings: settings(), ...patch });
  installCommands();
  return render(<SettingsWindow page="shortcuts" onPage={() => {}} />);
}

async function searchFor(text: string) {
  await userEvent.type(screen.getByRole("textbox", { name: "Search shortcuts" }), text);
}

const fields = () => screen.getAllByRole("button", { name: /Record shortcut|Press the new shortcut/ });
/** The search narrows the commands to one; the global shortcut's field comes first. */
const commandField = () => fields().at(-1)!;
const globalField = () => fields()[0];

describe("Settings Shortcuts page", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("global shortcut", () => {
    it("shows the saved start-timer shortcut under its group", () => {
      renderPage();
      expect(screen.getByText("Global shortcut")).toBeInTheDocument();
      expect(screen.getByText("Start timer (system-wide)")).toBeInTheDocument();
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
  });

  describe("removing a shortcut", () => {
    it("turns the global shortcut off and hides the X", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Remove shortcut for Start timer (system-wide)" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ timerShortcut: null }));
      await vi.waitFor(() => expect(globalField()).toHaveTextContent("unbound"));
      expect(screen.queryByRole("button", { name: "Remove shortcut for Start timer (system-wide)" })).not.toBeInTheDocument();
    });

    it("shows no X for an unbound global shortcut", () => {
      renderPage({ settings: settings({ timerShortcut: null }) });
      expect(screen.queryByRole("button", { name: "Remove shortcut for Start timer (system-wide)" })).not.toBeInTheDocument();
    });

    it("saves a null override for an in-app command", async () => {
      renderPage();
      await searchFor("Toggle sidebar");
      await userEvent.click(screen.getByRole("button", { name: "Remove shortcut for Toggle sidebar" }));
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": null });
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("unbound"));
      expect(screen.queryByRole("button", { name: "Remove shortcut for Toggle sidebar" })).not.toBeInTheDocument();
    });
  });

  it("groups the in-app commands under In TimeTracker", () => {
    renderPage();
    expect(screen.getByText("In TimeTracker")).toBeInTheDocument();
  });

  it("lists the commands with their bindings", async () => {
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

  it("saves the keys pressed while recording as the command's binding", async () => {
    renderPage();
    await searchFor("Toggle sidebar");
    await userEvent.click(commandField());
    await userEvent.keyboard("{Meta>}{Shift>}j{/Shift}{/Meta}");
    expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": "Mod+Shift+J" });
    await vi.waitFor(() => expect(commandField()).toHaveTextContent("⌘⇧J"));
  });

  it("unbinds the command when Backspace is pressed while recording", async () => {
    renderPage();
    await searchFor("Toggle sidebar");
    await userEvent.click(commandField());
    await userEvent.keyboard("{Backspace}");
    expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": null });
    await vi.waitFor(() => expect(commandField()).toHaveTextContent("unbound"));
  });

  it("keeps the binding when Escape cancels the recording", async () => {
    renderPage();
    await searchFor("Toggle sidebar");
    await userEvent.click(commandField());
    await userEvent.keyboard("{Escape}");
    expect(mocked.setKeybindings).not.toHaveBeenCalled();
    expect(commandField()).toHaveTextContent("⌘⌃S");
  });

  it("removes the override when the reset button is clicked", async () => {
    renderPage({ keybindings: { "sidebar.toggle": "Mod+Shift+J" } });
    await searchFor("Toggle sidebar");
    expect(commandField()).toHaveTextContent("⌘⇧J");
    await userEvent.click(screen.getByRole("button", { name: "Reset Toggle sidebar" }));
    expect(mocked.setKeybindings).toHaveBeenLastCalledWith({});
    await vi.waitFor(() => expect(commandField()).toHaveTextContent("⌘⌃S"));
  });

  it("warns when two commands share a binding", () => {
    renderPage({ keybindings: { "view.week": "Mod+1" } });
    expect(screen.getByRole("alert")).toHaveTextContent("⌘1 is used by both “Show Day” and “Show Week”.");
  });
});
