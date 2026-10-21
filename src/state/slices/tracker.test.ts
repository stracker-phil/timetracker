import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { trackerStatus } from "../../test/data";
import { resetStore, useStore } from "../store";

const mocked = vi.mocked(api, true);
const paused = trackerStatus({ state: { kind: "paused", until: null }, text: "Paused" });
const needsPermission = trackerStatus({ state: { kind: "needsPermission" }, permission: "denied", text: "Needs permission" });
const toastMessages = () => useStore.getState().toasts.map((t) => t.message);

describe("tracker slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  describe("loadTracker", () => {
    it("stores the status the backend reports", async () => {
      mocked.trackerStatus.mockResolvedValueOnce(paused);
      await useStore.getState().loadTracker();
      expect(useStore.getState().tracker).toEqual(paused);
    });

    it("leaves the tracker off when the status cannot be read", async () => {
      resetStore({ tracker: trackerStatus() });
      mocked.trackerStatus.mockRejectedValueOnce({ kind: "unavailable", message: "No data store", code: null });
      await useStore.getState().loadTracker();
      expect(useStore.getState().tracker).toBeNull();
    });
  });

  describe("permission dialog visibility", () => {
    it("stays hidden while the permission is still needed", () => {
      resetStore({ tracker: needsPermission, permissionDialogHidden: true });
      useStore.getState().setTracker(needsPermission);
      expect(useStore.getState().permissionDialogHidden).toBe(true);
    });

    it("is reset once the tracker is no longer waiting for the permission", () => {
      resetStore({ tracker: needsPermission, permissionDialogHidden: true });
      useStore.getState().setTracker(trackerStatus());
      expect(useStore.getState().permissionDialogHidden).toBe(false);
    });

    it("is reset by a status loaded from the backend", async () => {
      resetStore({ tracker: needsPermission, permissionDialogHidden: true });
      await useStore.getState().loadTracker();
      expect(useStore.getState().permissionDialogHidden).toBe(false);
    });
  });

  describe("pauseTracker", () => {
    it("sends the choice and stores the returned status", async () => {
      mocked.trackerPause.mockResolvedValueOnce(paused);
      await useStore.getState().pauseTracker({ minutes: 60 });
      expect(mocked.trackerPause).toHaveBeenCalledWith({ minutes: 60 });
      expect(useStore.getState().tracker).toEqual(paused);
    });

    it("keeps the status and shows a toast when pausing fails", async () => {
      resetStore({ tracker: trackerStatus() });
      mocked.trackerPause.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await useStore.getState().pauseTracker("tomorrow");
      expect(useStore.getState().tracker).toEqual(trackerStatus());
      expect(toastMessages()).toContain("Could not pause tracking: Disk full");
    });
  });

  describe("resumeTracker", () => {
    it("stores the returned status", async () => {
      resetStore({ tracker: paused });
      await useStore.getState().resumeTracker();
      expect(useStore.getState().tracker).toEqual(trackerStatus());
    });

    it("shows a toast when resuming fails", async () => {
      resetStore({ tracker: paused });
      mocked.trackerResume.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await useStore.getState().resumeTracker();
      expect(useStore.getState().tracker).toEqual(paused);
      expect(toastMessages()).toContain("Could not resume tracking: Disk full");
    });
  });

  describe("requestPermission", () => {
    it("reloads the status once the permission is granted", async () => {
      resetStore({ tracker: needsPermission });
      await useStore.getState().requestPermission();
      expect(useStore.getState().tracker).toEqual(trackerStatus());
      expect(toastMessages()).toEqual([]);
    });

    it("tells the user when the permission is still missing", async () => {
      mocked.requestPermission.mockResolvedValueOnce("denied");
      mocked.trackerStatus.mockResolvedValueOnce(needsPermission);
      await useStore.getState().requestPermission();
      expect(toastMessages()).toContain("Accessibility is not granted yet");
      expect(useStore.getState().tracker).toEqual(needsPermission);
    });

    it("shows a toast when the permission cannot be checked", async () => {
      mocked.requestPermission.mockRejectedValueOnce({ kind: "io", message: "Denied by system", code: null });
      await useStore.getState().requestPermission();
      expect(toastMessages()).toContain("Could not check the permission: Denied by system");
    });
  });
});
