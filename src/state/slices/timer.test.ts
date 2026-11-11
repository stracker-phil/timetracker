import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { today } from "../../lib/day";
import { recording } from "../../test/recordingFixtures";
import { resetStore, useStore } from "../store";

const mocked = vi.mocked(api, true);
const toasts = () => useStore.getState().toasts;
const toastMessages = () => toasts().map((t) => t.message);
const stopped = () => ({ kind: "recording" as const, ...recording() });

describe("timer slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  describe("startTimer", () => {
    it("sends project, note and tag and resolves true", async () => {
      await expect(useStore.getState().startTimer("p1", "standup", "t1")).resolves.toBe(true);
      expect(mocked.timerStart).toHaveBeenCalledWith("p1", "standup", "t1");
      expect(toasts()).toEqual([]);
    });

    it("toasts that a recording covers now on an overlap", async () => {
      mocked.timerStart.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps", code: null });
      await expect(useStore.getState().startTimer("p1", "", null)).resolves.toBe(false);
      expect(toastMessages()).toEqual(["A recording already covers now"]);
    });

    it("toasts the message of any other error", async () => {
      mocked.timerStart.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await expect(useStore.getState().startTimer("p1", "", null)).resolves.toBe(false);
      expect(toastMessages()).toContain("Could not start the timer: Disk full");
    });
  });

  describe("updateTimer", () => {
    it("sends project, note and tag and resolves true", async () => {
      await expect(useStore.getState().updateTimer("p2", "review", "t2")).resolves.toBe(true);
      expect(mocked.timerUpdate).toHaveBeenCalledWith("p2", "review", "t2");
      expect(toasts()).toEqual([]);
    });

    it("resolves false without a toast when no timer runs", async () => {
      mocked.timerUpdate.mockResolvedValueOnce(null);
      await expect(useStore.getState().updateTimer("p2", "", null)).resolves.toBe(false);
      expect(toasts()).toEqual([]);
    });

    it("toasts that the timer could not be changed when the call fails", async () => {
      mocked.timerUpdate.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await expect(useStore.getState().updateTimer("p2", "", null)).resolves.toBe(false);
      expect(toastMessages()).toContain("Could not change the timer: Disk full");
    });
  });

  describe("stopTimer", () => {
    it("toasts Timer stopped with Undo when a recording came back", async () => {
      mocked.timerStop.mockResolvedValueOnce(stopped());
      await useStore.getState().stopTimer();
      expect(toastMessages()).toEqual(["Timer stopped"]);
      expect(toasts()[0].action?.label).toBe("Undo");
    });

    it("toasts Timer stopped when a suggested timer came back", async () => {
      mocked.timerStop.mockResolvedValueOnce({ kind: "suggestion", id: "st-timer", projectId: null, note: "", tagId: null, source: "cli", day: today(), spans: [], createdAt: 0, updatedAt: 0 });
      await useStore.getState().stopTimer();
      expect(toastMessages()).toEqual(["Timer stopped"]);
    });

    it("runs the Undo action through the api", async () => {
      mocked.timerStop.mockResolvedValueOnce(stopped());
      await useStore.getState().stopTimer();
      toasts()[0].action?.run();
      await vi.waitFor(() => expect(mocked.undo).toHaveBeenCalled());
    });

    it("shows nothing when no timer ran", async () => {
      await useStore.getState().stopTimer();
      expect(toasts()).toEqual([]);
    });

    it("toasts the message when stopping fails", async () => {
      mocked.timerStop.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await useStore.getState().stopTimer();
      expect(toastMessages()).toContain("Could not stop the timer: Disk full");
    });
  });
});
