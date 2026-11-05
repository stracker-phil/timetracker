import { api, type PauseUntil, type TrackerStatus } from "../../api";
import { queryClient } from "../../queries/queryClient";
import { fetchTracker, TRACKER_KEY } from "../../queries/status";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface TrackerSlice {
  /** The permission dialog was closed; it shows again when asked for or when the permission is lost anew. */
  permissionDialogHidden: boolean;
  setPermissionDialogHidden: (hidden: boolean) => void;
  /** A status from Rust (an answer or `tracker_status`), written into the tracker's query (`useTracker`). */
  setTracker: (status: TrackerStatus | null) => void;
  pauseTracker: (until: PauseUntil) => Promise<void>;
  resumeTracker: () => Promise<void>;
  /** Asks macOS for Accessibility, then shows whether it is granted now. */
  requestPermission: () => Promise<void>;
}

export const trackerSlice: Slice<TrackerSlice> = {
  create: (set, get) => ({
    permissionDialogHidden: false,

    setTracker(tracker) {
      queryClient.setQueryData(TRACKER_KEY, tracker);
      set((s) => ({ permissionDialogHidden: s.permissionDialogHidden && tracker?.state.kind === "needsPermission" }));
    },

    setPermissionDialogHidden(permissionDialogHidden) {
      set({ permissionDialogHidden });
    },

    async pauseTracker(until) {
      try {
        queryClient.setQueryData(TRACKER_KEY, await api.trackerPause(until));
      } catch (e) {
        fail(e, "Could not pause tracking");
      }
    },

    async resumeTracker() {
      try {
        queryClient.setQueryData(TRACKER_KEY, await api.trackerResume());
      } catch (e) {
        fail(e, "Could not resume tracking");
      }
    },

    async requestPermission() {
      try {
        const permission = await api.requestPermission();
        if (permission !== "granted") get().toast("info", "Accessibility is not granted yet");
        get().setTracker(await fetchTracker());
      } catch (e) {
        fail(e, "Could not check the permission");
      }
    },
  }),
};
