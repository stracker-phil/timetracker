import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { block, dayView, lane } from "../../test/dayFixtures";
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

  describe("loadDay", () => {
    it("stores the view of the shown day", async () => {
      const view = dayView({ day: "2026-03-10", lanes: [lane({ blocks: [block()] })] });
      mocked.dayView.mockResolvedValueOnce(view);
      await state().loadDay();
      expect(mocked.dayView).toHaveBeenCalledWith("2026-03-10");
      expect(state().dayView).toEqual(view);
    });

    it("drops a response that arrives after a newer request", async () => {
      let resolveOld!: (v: ReturnType<typeof dayView>) => void;
      mocked.dayView.mockReturnValueOnce(new Promise((r) => (resolveOld = r)));
      const old = state().loadDay();
      useStore.setState({ day: "2026-03-11" });
      await state().loadDay();
      resolveOld(dayView({ day: "2026-03-10" }));
      await old;
      expect(state().dayView?.day).toBe("2026-03-11");
    });

    it("keeps the previous view while another day loads", async () => {
      await state().loadDay();
      let resolveNext!: (v: ReturnType<typeof dayView>) => void;
      mocked.dayView.mockReturnValueOnce(new Promise((r) => (resolveNext = r)));
      useStore.setState({ day: "2026-03-11" });
      const loading = state().loadDay();
      expect(state().dayView?.day).toBe("2026-03-10");
      resolveNext(dayView({ day: "2026-03-11" }));
      await loading;
      expect(state().dayView?.day).toBe("2026-03-11");
    });

    it("keeps the focused block when the same day reloads", async () => {
      await state().loadDay();
      state().focusBlock("a1");
      await state().loadDay();
      expect(state().focusedBlock).toBe("a1");
    });

    it("clears the focus when another day arrives", async () => {
      await state().loadDay();
      state().focusBlock("a1");
      useStore.setState({ day: "2026-03-11" });
      await state().loadDay();
      expect(state().focusedBlock).toBeNull();
    });

    it("stores the error when the day cannot be loaded", async () => {
      mocked.dayView.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      await state().loadDay();
      expect(state().dayError?.message).toBe("Database locked");
    });

    it("clears an earlier error once a load succeeds", async () => {
      mocked.dayView.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      await state().loadDay();
      await state().loadDay();
      expect(state().dayError).toBeNull();
    });
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
      expect(state().settings).toEqual(saved);
    });

    it("resolves true when the patch was saved", async () => {
      await expect(state().setAppSetting("com.apple.Safari", { hidden: true })).resolves.toBe(true);
    });

    it("resolves false, shows a toast and keeps the settings when saving fails", async () => {
      const before = settings();
      resetStore({ settings: before });
      mocked.setAppSetting.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await expect(state().setAppSetting("com.apple.Safari", { ownLane: true })).resolves.toBe(false);
      expect(state().settings).toEqual(before);
      expect(toastMessages()).toContain("Could not change the app's timeline setting: Disk full");
    });
  });
});
