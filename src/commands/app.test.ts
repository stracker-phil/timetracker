import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore } from "../state/store";
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

  describe("app.background", () => {
    it("hides to the menu bar", () => {
      expect(runCommand("app.background")).toBe(true);
      expect(mocked.hideToMenuBar).toHaveBeenCalled();
      expect(mocked.quitApp).not.toHaveBeenCalled();
    });

    it("runs on Cmd+Q instead of quitting", async () => {
      await press("{Meta>}q{/Meta}");
      expect(mocked.hideToMenuBar).toHaveBeenCalled();
      expect(mocked.quitApp).not.toHaveBeenCalled();
    });

    it("still runs on Cmd+Q while a dialog is open", async () => {
      resetStore({ dialog: { type: "settings" } });
      await press("{Meta>}q{/Meta}");
      expect(mocked.hideToMenuBar).toHaveBeenCalled();
    });
  });

  describe("app.quit", () => {
    it("quits the app", () => {
      expect(runCommand("app.quit")).toBe(true);
      expect(mocked.quitApp).toHaveBeenCalled();
    });

    it("has no key binding", () => {
      expect(bindingFor("app.quit")).toBeNull();
      expect(bindingFor("app.background")).toBe("Mod+Q");
    });
  });
});
