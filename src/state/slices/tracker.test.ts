import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type TrackerStatus } from "../../api";
import { trackerStatus } from "../../test/data";
import { queryData, seedQuery, TRACKER_KEY } from "../../test/queries";
import { resetStore, useStore } from "../store";

const mocked = vi.mocked(api, true);
const paused = trackerStatus({ state: { kind: "paused", until: null }, text: "Paused" });
const needsPermission = trackerStatus({ state: { kind: "needsPermission" }, permission: "denied", text: "Needs permission" });
const toastMessages = () => useStore.getState().toasts.map((t) => t.message);
const cached = () => queryData<TrackerStatus | null>(TRACKER_KEY);

describe("tracker slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  describe("setTracker", () => {
    it("writes the status into the tracker's query", () => {
      useStore.getState().setTracker(paused);
      expect(cached()).toEqual(paused);
    });

    it("writes null when the tracker is off", () => {
      seedQuery(TRACKER_KEY, paused);
      useStore.getState().setTracker(null);
      expect(cached()).toBeNull();
    });
  });

  describe("permission dialog visibility", () => {
    it("stays hidden while the permission is still needed", () => {
      resetStore({ permissionDialogHidden: true });
      useStore.getState().setTracker(needsPermission);
      expect(useStore.getState().permissionDialogHidden).toBe(true);
    });

    it("is reset once the tracker is no longer waiting for the permission", () => {
      resetStore({ permissionDialogHidden: true });
      useStore.getState().setTracker(trackerStatus());
      expect(useStore.getState().permissionDialogHidden).toBe(false);
    });

    it("is reset when the tracker is off", () => {
      resetStore({ permissionDialogHidden: true });
      useStore.getState().setTracker(null);
      expect(useStore.getState().permissionDialogHidden).toBe(false);
    });

    it("is set by setPermissionDialogHidden", () => {
      useStore.getState().setPermissionDialogHidden(true);
      expect(useStore.getState().permissionDialogHidden).toBe(true);
    });
  });

  describe("pauseTracker", () => {
    it("sends the choice and stores the returned status", async () => {
      mocked.trackerPause.mockResolvedValueOnce(paused);
      await useStore.getState().pauseTracker({ minutes: 60 });
      expect(mocked.trackerPause).toHaveBeenCalledWith({ minutes: 60 });
      expect(cached()).toEqual(paused);
    });

    it("keeps the status and shows a toast when pausing fails", async () => {
      seedQuery(TRACKER_KEY, trackerStatus());
      mocked.trackerPause.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await useStore.getState().pauseTracker("tomorrow");
      expect(cached()).toEqual(trackerStatus());
      expect(toastMessages()).toContain("Could not pause tracking: Disk full");
    });
  });

  describe("resumeTracker", () => {
    it("stores the returned status", async () => {
      seedQuery(TRACKER_KEY, paused);
      await useStore.getState().resumeTracker();
      expect(cached()).toEqual(trackerStatus());
    });

    it("shows a toast and keeps the status when resuming fails", async () => {
      seedQuery(TRACKER_KEY, paused);
      mocked.trackerResume.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await useStore.getState().resumeTracker();
      expect(cached()).toEqual(paused);
      expect(toastMessages()).toContain("Could not resume tracking: Disk full");
    });
  });

  describe("requestPermission", () => {
    it("shows the status Rust reports once the permission is granted", async () => {
      seedQuery(TRACKER_KEY, needsPermission);
      await useStore.getState().requestPermission();
      expect(cached()).toEqual(trackerStatus());
      expect(toastMessages()).toEqual([]);
    });

    it("asks the status again instead of trusting the cached one", async () => {
      seedQuery(TRACKER_KEY, paused);
      mocked.trackerStatus.mockResolvedValueOnce(trackerStatus({ text: "Tracking again" }));
      await useStore.getState().requestPermission();
      expect(cached()?.text).toBe("Tracking again");
    });

    it("tells the user when the permission is still missing", async () => {
      mocked.requestPermission.mockResolvedValueOnce("denied");
      mocked.trackerStatus.mockResolvedValueOnce(needsPermission);
      await useStore.getState().requestPermission();
      expect(toastMessages()).toContain("Accessibility is not granted yet");
      expect(cached()).toEqual(needsPermission);
    });

    it("shows a toast when the permission cannot be checked", async () => {
      mocked.requestPermission.mockRejectedValueOnce({ kind: "io", message: "Denied by system", code: null });
      await useStore.getState().requestPermission();
      expect(toastMessages()).toContain("Could not check the permission: Denied by system");
    });
  });
});
