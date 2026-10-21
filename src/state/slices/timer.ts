import { api, toAppError, type TimerStatus } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface TimerSlice {
  /** `null` until the first status arrives. */
  timer: TimerStatus | null;
  setTimerStatus: (status: TimerStatus) => void;
  loadTimer: () => Promise<void>;
  /** Resolves whether it started. */
  startTimer: (projectId: string, note: string, tagId: string | null) => Promise<boolean>;
  stopTimer: () => Promise<void>;
}

export const timerSlice: Slice<TimerSlice> = {
  create: (set, get) => ({
    timer: null,

    setTimerStatus(timer) {
      set({ timer });
    },

    async loadTimer() {
      try {
        set({ timer: await api.timerStatus() });
      } catch {
        // The tray and the next event still show it.
      }
    },

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
