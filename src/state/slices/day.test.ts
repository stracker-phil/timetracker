import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Settings } from "../../api";
import { queryData, seedQuery, SETTINGS_KEY } from "../../test/queries";
import { settings } from "../../test/settings";
import { resetStore, useStore } from "../store";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();
const toastMessages = () => state().toasts.map((t) => t.message);

describe("day slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ day: "2026-03-10" });
  });

  describe("details", () => {
    it("opens the focused block's details and closes them on a second toggle", () => {
      state().focusBlock("a2");
      state().toggleDetails();
      expect(state().detailsBlock).toBe("a2");
      state().toggleDetails();
      expect(state().detailsBlock).toBeNull();
    });

    it("closes the details when the focus moves", () => {
      state().focusBlock("a2");
      state().toggleDetails();
      state().focusBlock("a3");
      expect(state().detailsBlock).toBeNull();
    });
  });

  describe("focusBlock", () => {
    it("takes the focus off a suggestion when a block gets it", () => {
      resetStore({ focusedSuggestion: "s1" });
      state().focusBlock("a1");
      expect([state().focusedBlock, state().focusedSuggestion]).toEqual(["a1", null]);
    });

    it("leaves the suggestion focus alone when it clears the block", () => {
      resetStore({ focusedBlock: "a1", focusedSuggestion: "s1" });
      state().focusBlock(null);
      expect([state().focusedBlock, state().focusedSuggestion]).toEqual([null, "s1"]);
    });
  });

  describe("requestBlockMenu", () => {
    it("asks for the menu only while a block is focused", () => {
      state().requestBlockMenu();
      expect(state().blockMenuRequest).toBe(0);
      state().focusBlock("a1");
      state().requestBlockMenu();
      expect(state().blockMenuRequest).toBe(1);
    });
  });

  describe("setAppSetting", () => {
    it("sends the patch and stores the settings it returns", async () => {
      const saved = settings({ accent: "teal" });
      mocked.setAppSetting.mockResolvedValueOnce(saved);
      await state().setAppSetting("com.apple.Safari", { hidden: true });
      expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Safari", { hidden: true });
      expect(queryData<Settings>(SETTINGS_KEY)).toEqual(saved);
    });

    it("resolves true when the patch was saved", async () => {
      await expect(state().setAppSetting("com.apple.Safari", { hidden: true })).resolves.toBe(true);
    });

    it("resolves false, shows a toast and keeps the settings when saving fails", async () => {
      const before = settings();
      seedQuery(SETTINGS_KEY, before);
      mocked.setAppSetting.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await expect(state().setAppSetting("com.apple.Safari", { ownLane: true })).resolves.toBe(false);
      expect(queryData<Settings>(SETTINGS_KEY)).toEqual(before);
      expect(toastMessages()).toContain("Could not change the app's timeline setting: Disk full");
    });
  });
});
