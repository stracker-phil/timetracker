import { api, toAppError } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface TimerSlice {
  /** Resolves whether it started. */
  startTimer: (projectId: string, note: string, tagId: string | null) => Promise<boolean>;
  /** Gives the running timer another project, note and tag; resolves whether it saved. */
  updateTimer: (projectId: string, note: string, tagId: string | null) => Promise<boolean>;
  stopTimer: () => Promise<void>;
}

export const timerSlice: Slice<TimerSlice> = {
  create: (_set, get) => ({
    async startTimer(projectId, note, tagId) {
      try {
        await api.timerStart(projectId, note, tagId);
        return true;
      } catch (e) {
        if (toAppError(e).kind === "overlap") get().toast("error", "A recording already covers now");
        else fail(e, "Could not start the timer");
        return false;
      }
    },

    async updateTimer(projectId, note, tagId) {
      try {
        return (await api.timerUpdate(projectId, note, tagId)) !== null;
      } catch (e) {
        fail(e, "Could not change the timer");
        return false;
      }
    },

    async stopTimer() {
      try {
        const stopped = await api.timerStop();
        if (stopped) get().toast("success", "Timer stopped", { label: "Undo", run: () => void get().undo() });
      } catch (e) {
        fail(e, "Could not stop the timer");
      }
    },
  }),
};
