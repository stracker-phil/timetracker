import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { markSettingsWindow } from "../lib/settingsWindow";
import { resetStore } from "../state/store";
import { installCommands } from "./index";
import { handleSettingsKeydown } from "./settingsWindow";

const mocked = vi.mocked(api, true);

async function press(keys: string) {
  window.addEventListener("keydown", handleSettingsKeydown, true);
  try {
    await userEvent.keyboard(keys);
  } finally {
    window.removeEventListener("keydown", handleSettingsKeydown, true);
  }
}

describe("Settings window commands", () => {
  let leave: () => void;

  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    installCommands();
    leave = markSettingsWindow();
  });

  afterEach(() => {
    leave();
    document.body.innerHTML = "";
  });

  it("settings.close hides the window on Escape", async () => {
    await press("{Escape}");
    expect(mocked.hideWindow).toHaveBeenCalled();
  });

  it("keeps the window open when Escape is pressed in a text field", async () => {
    const field = document.body.appendChild(document.createElement("input"));
    field.focus();
    await press("{Escape}");
    expect(mocked.hideWindow).not.toHaveBeenCalled();
  });

  it("keeps the window open when Escape is pressed with a sheet open", async () => {
    const sheet = document.body.appendChild(document.createElement("div"));
    sheet.setAttribute("role", "dialog");
    await press("{Escape}");
    expect(mocked.hideWindow).not.toHaveBeenCalled();
  });

  it("settings.search focuses the search field on Cmd+F", async () => {
    const search = document.body.appendChild(document.createElement("input"));
    search.setAttribute("aria-label", "Search Settings");
    await press("{Meta>}f{/Meta}");
    expect(search).toHaveFocus();
  });
});
