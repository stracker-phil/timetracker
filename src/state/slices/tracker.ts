import { api, type PauseUntil, type TrackerStatus } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface TrackerSlice {
  /** `null` until loaded, and while the tracker is off (no data store). */
  tracker: TrackerStatus | null;
  /** The permission dialog was closed; it shows again when asked for or when the permission is lost anew. */
  permissionDialogHidden: boolean;
  setPermissionDialogHidden: (hidden: boolean) => void;
  setTracker: (status: TrackerStatus) => void;
  loadTracker: () => Promise<void>;
  pauseTracker: (until: PauseUntil) => Promise<void>;
  resumeTracker: () => Promise<void>;
  /** Asks macOS for Accessibility, then shows whether it is granted now. */
  requestPermission: () => Promise<void>;
}

export const trackerSlice: Slice<TrackerSlice> = {
  create: (set, get) => ({
    tracker: null,
    permissionDialogHidden: false,

    setTracker(tracker) {
      set((s) => ({ tracker, permissionDialogHidden: s.permissionDialogHidden && tracker.state.kind === "needsPermission" }));
    },

    setPermissionDialogHidden(permissionDialogHidden) {
      set({ permissionDialogHidden });
    },

    async loadTracker() {
      try {
        get().setTracker(await api.trackerStatus());
      } catch {
        // The tracker is off without a data store; Settings → Data says why.
        set({ tracker: null });
      }
    },

    async pauseTracker(until) {
      try {
        set({ tracker: await api.trackerPause(until) });
      } catch (e) {
        fail(e, "Could not pause tracking");
      }
    },

    async resumeTracker() {
      try {
        set({ tracker: await api.trackerResume() });
      } catch (e) {
        fail(e, "Could not resume tracking");
      }
    },

    async requestPermission() {
      try {
        const permission = await api.requestPermission();
        if (permission !== "granted") get().toast("info", "Accessibility is not granted yet");
        await get().loadTracker();
      } catch (e) {
        fail(e, "Could not check the permission");
      }
    },
  }),
};
