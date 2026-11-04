import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Keybindings, type Settings } from "../../api";
import { bindingLabel } from "../../commands";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsApp } from "./SettingsApp";

const mocked = vi.mocked(api, true);

const heading = () => screen.findByRole("heading", { level: 2 });

async function renderAt(query = "") {
  window.history.replaceState(null, "", `/${query}`);
  render(<SettingsApp />);
  await heading();
}

/** The handler the app registered for an event, to play the event Rust would emit. */
async function emit<T>(listener: { mock: { lastCall?: unknown[] } }, payload: T) {
  await vi.waitFor(() => expect(listener.mock.lastCall).toBeDefined());
  const handler = listener.mock.lastCall![0] as (payload: T) => void;
  await act(async () => handler(payload));
}

describe("SettingsApp", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    mocked.getSettings.mockResolvedValue(settings());
  });

  afterEach(() => {
    window.history.replaceState(null, "", "/");
    Reflect.deleteProperty(Element.prototype, "scrollIntoView");
  });

  describe("opening page", () => {
    it("opens at the first page without a page parameter", async () => {
      await renderAt();
      expect(await heading()).toHaveTextContent("General");
    });

    it("opens at the page named by ?page=", async () => {
      await renderAt("?page=capture");
      expect(await heading()).toHaveTextContent("Capture");
    });

    it("opens at the page holding a section named by ?page= and scrolls to it", async () => {
      const scrollIntoView = vi.fn();
      Element.prototype.scrollIntoView = scrollIntoView;
      await renderAt("?page=away");
      expect(await heading()).toHaveTextContent("Tracker");
      expect(scrollIntoView.mock.contexts).toHaveLength(1);
      expect(scrollIntoView.mock.contexts[0]).toContainElement(screen.getByLabelText("Away after"));
    });

    it("switches page when Rust sends open_settings", async () => {
      await renderAt("?page=capture");
      await emit(mocked.onOpenSettings, "shortcuts");
      expect(await screen.findByRole("heading", { level: 2, name: "Shortcuts" })).toBeInTheDocument();
    });
  });

  describe("following changes from other windows", () => {
    it("shows settings saved elsewhere (settings_changed)", async () => {
      await renderAt("?page=capture");
      expect(screen.getByRole("switch", { name: "Capture document paths" })).toBeChecked();
      const changed: Settings = settings({ capture: { ...settings().capture, captureDocuments: false } });
      await emit(mocked.onSettingsChanged, changed);
      expect(screen.getByRole("switch", { name: "Capture document paths" })).not.toBeChecked();
    });

    it("shows rebound shortcuts (keybindings_changed) on the Shortcuts page", async () => {
      await renderAt("?page=shortcuts");
      expect(screen.queryByRole("button", { name: "Reset Search Settings" })).not.toBeInTheDocument();
      const binding = "Mod+Alt+Shift+J";
      const rebound: Keybindings = { "settings.search": binding };
      await emit(mocked.onKeybindingsChanged, rebound);
      const reset = screen.getByRole("button", { name: "Reset Search Settings" });
      expect(within(reset.parentElement!).getByText(bindingLabel(binding))).toBeInTheDocument();
    });
  });

  describe("keys", () => {
    it("focuses the search field on ⌘F", async () => {
      await renderAt();
      await vi.waitFor(async () => {
        await userEvent.keyboard("{Meta>}f{/Meta}");
        expect(screen.getByRole("searchbox", { name: "Search Settings" })).toHaveFocus();
      });
    });

    it("hides the window on Escape", async () => {
      await renderAt();
      await vi.waitFor(async () => {
        await userEvent.keyboard("{Escape}");
        expect(mocked.hideWindow).toHaveBeenCalled();
      });
    });

    it("leaves Escape to a focused text field", async () => {
      await renderAt();
      await vi.waitFor(async () => {
        await userEvent.keyboard("{Meta>}f{/Meta}");
        expect(screen.getByRole("searchbox", { name: "Search Settings" })).toHaveFocus();
      });
      await userEvent.keyboard("{Escape}");
      expect(mocked.hideWindow).not.toHaveBeenCalled();
    });
  });

  describe("General page", () => {
    it("asks Rust to show the welcome sheet again", async () => {
      await renderAt("?page=general");
      await userEvent.click(screen.getByRole("button", { name: "Show welcome again" }));
      expect(mocked.showWelcome).toHaveBeenCalled();
    });
  });

  describe("window focus", () => {
    it("dims the page while the window is not key and restores it when it is", async () => {
      const hasFocus = vi.spyOn(document, "hasFocus").mockReturnValue(true);
      try {
        await renderAt();
        const root = document.documentElement;
        expect(root).not.toHaveClass("window-blurred");
        hasFocus.mockReturnValue(false);
        act(() => void window.dispatchEvent(new Event("blur")));
        expect(root).toHaveClass("window-blurred");
        hasFocus.mockReturnValue(true);
        act(() => void window.dispatchEvent(new Event("focus")));
        expect(root).not.toHaveClass("window-blurred");
      } finally {
        hasFocus.mockRestore();
      }
    });
  });
});
