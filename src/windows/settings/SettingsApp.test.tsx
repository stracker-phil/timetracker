import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Keybindings, type Settings } from "../../api";
import { bindingLabel } from "../../commands";
import { resetStore } from "../../state/store";
import { emit } from "../../test/apiMock";
import { settings } from "../../test/settings";
import { SettingsApp } from "./SettingsApp";

const mocked = vi.mocked(api, true);

const heading = () => screen.findByRole("heading", { level: 2 });
const rowOf = (label: string) => screen.getByLabelText(label).closest<HTMLElement>(".settings-row")!;

async function renderAt(query = "") {
  window.history.replaceState(null, "", `/${query}`);
  render(<SettingsApp />);
  await heading();
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

    it("opens at the page holding a row named by ?page=, scrolled to the row and washed", async () => {
      const scrollIntoView = vi.fn();
      Element.prototype.scrollIntoView = scrollIntoView;
      await renderAt("?page=away-after");
      expect(await heading()).toHaveTextContent("Tracker");
      expect(scrollIntoView.mock.contexts).toEqual([rowOf("Away after")]);
      expect(rowOf("Away after")).toHaveClass("found");
    });

    it("switches page when Rust sends open_settings", async () => {
      await renderAt("?page=capture");
      emit("open_settings", "shortcuts");
      expect(await screen.findByRole("heading", { level: 2, name: "Shortcuts" })).toBeInTheDocument();
    });

    it("opens the page of a row Rust names in open_settings and washes the row", async () => {
      const scrollIntoView = vi.fn();
      Element.prototype.scrollIntoView = scrollIntoView;
      await renderAt("?page=capture");
      emit("open_settings", "away-after");
      expect(await screen.findByRole("heading", { level: 2, name: "Tracker" })).toBeInTheDocument();
      expect(scrollIntoView.mock.contexts).toEqual([rowOf("Away after")]);
      expect(rowOf("Away after")).toHaveClass("found");
    });

    it("washes the row again when Rust sends the same open_settings twice", async () => {
      const scrollIntoView = vi.fn();
      Element.prototype.scrollIntoView = scrollIntoView;
      await renderAt("?page=capture");
      emit("open_settings", "away-after");
      await screen.findByRole("heading", { level: 2, name: "Tracker" });
      emit("open_settings", "away-after");
      expect(scrollIntoView.mock.contexts).toEqual([rowOf("Away after"), rowOf("Away after")]);
    });
  });

  describe("following changes from other windows", () => {
    it("shows settings saved elsewhere (settings_changed)", async () => {
      await renderAt("?page=capture");
      expect(screen.getByRole("switch", { name: "Capture document paths" })).toBeChecked();
      const changed: Settings = settings({ capture: { ...settings().capture, captureDocuments: false } });
      emit("settings_changed", changed);
      await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Capture document paths" })).not.toBeChecked());
    });

    it("shows rebound shortcuts (keybindings_changed) on the Shortcuts page", async () => {
      await renderAt("?page=shortcuts");
      expect(screen.queryByRole("button", { name: "Reset Search Settings" })).not.toBeInTheDocument();
      const binding = "Mod+Alt+Shift+J";
      const rebound: Keybindings = { "settings.search": binding };
      emit("keybindings_changed", rebound);
      const reset = await screen.findByRole("button", { name: "Reset Search Settings" });
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
