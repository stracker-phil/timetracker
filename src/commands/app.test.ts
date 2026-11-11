import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore, useStore } from "../state/store";
import { handleKeydown, installCommands, runCommand } from "./index";
import { bindingFor } from "./registry";

const mocked = vi.mocked(api, true);

async function press(keys: string) {
  window.addEventListener("keydown", handleKeydown, true);
  try {
    await userEvent.keyboard(keys);
  } finally {
    window.removeEventListener("keydown", handleKeydown, true);
  }
}

describe("app commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    installCommands();
  });

  describe("palette.reveal", () => {
    it("opens the palette seeded with the recordings prefix on Cmd+Shift+F", async () => {
      await press("{Meta>}{Shift>}f{/Shift}{/Meta}");
      expect(useStore.getState().paletteOpen).toBe(true);
      expect(useStore.getState().paletteSeed).toBe("r: ");
    });
  });

  describe("app:background", () => {
    it("asks the bus to hide to the menu bar", () => {
      expect(runCommand("app:background")).toBe(true);
      expect(mocked.runCommand).toHaveBeenCalledWith("app:background");
      expect(mocked.runCommand).not.toHaveBeenCalledWith("app:quit");
    });

    it("runs on Cmd+Q instead of quitting", async () => {
      await press("{Meta>}q{/Meta}");
      expect(mocked.runCommand).toHaveBeenCalledWith("app:background");
      expect(mocked.runCommand).not.toHaveBeenCalledWith("app:quit");
    });

    it("still runs on Cmd+Q while a dialog is open", async () => {
      resetStore({ dialog: { type: "shortcuts", scopes: ["global"] } });
      await press("{Meta>}q{/Meta}");
      expect(mocked.runCommand).toHaveBeenCalledWith("app:background");
    });
  });

  describe("app:settings", () => {
    it("asks the bus to open the Settings window", () => {
      expect(runCommand("app:settings")).toBe(true);
      expect(mocked.runCommand).toHaveBeenCalledWith("app:settings");
    });

    it("opens it on Cmd+Comma", async () => {
      await press("{Meta>},{/Meta}");
      expect(mocked.runCommand).toHaveBeenCalledWith("app:settings");
    });

    it("shows an error toast when the bus refuses", async () => {
      mocked.runCommand.mockRejectedValueOnce({ kind: "io", message: "No window", code: null });
      runCommand("app:settings");
      await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message)).toEqual(["Couldn't run Settings…: No window"]));
    });
  });

  describe("window:close", () => {
    it("closes the window on Cmd+W", async () => {
      await press("{Meta>}w{/Meta}");
      expect(mocked.runCommand).toHaveBeenCalledWith("window:close");
    });
  });

  describe("app:quit", () => {
    it("asks the bus to quit", () => {
      expect(runCommand("app:quit")).toBe(true);
      expect(mocked.runCommand).toHaveBeenCalledWith("app:quit");
    });

    it("has no key binding", () => {
      expect(bindingFor("app:quit")).toBeNull();
      expect(bindingFor("app:background")).toBe("Mod+Q");
    });
  });
});
