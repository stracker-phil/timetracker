import { api, toAppError } from "../../api";
import { today } from "../../lib/day";
import { projectLists } from "../../queries/projects";
import { currentTimer } from "../../queries/status";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface TimerSlice {
  /** Resolves whether it started. */
  startTimer: (projectId: string, note: string, tagId: string | null) => Promise<boolean>;
  /** Gives the running timer another project, note and tag; resolves whether it saved. */
  updateTimer: (projectId: string, note: string, tagId: string | null) => Promise<boolean>;
  /** Approves the suggested timer with this project, note and tag: it keeps running as a recording (ADR 0041).
   *  Resolves whether it did. */
  approveTimer: (projectId: string, note: string, tagId: string | null) => Promise<boolean>;
  stopTimer: () => Promise<void>;
}

const COVERED = "A recording already covers now";

export const timerSlice: Slice<TimerSlice> = {
  create: (_set, get) => ({
    async startTimer(projectId, note, tagId) {
      try {
        await api.timerStart(projectId, note, tagId);
        return true;
      } catch (e) {
        if (toAppError(e).kind === "overlap") get().toast("error", COVERED);
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

    async approveTimer(projectId, note, tagId) {
      const running = currentTimer()?.running;
      if (!running?.suggested) return false;
      try {
        // Today's view holds the piece that reaches now, whichever day the timer started on.
        const day = today();
        const piece = (await api.dayView(day)).suggestions.find((s) => s.running && s.storedId === running.id);
        if (!piece) {
          get().toast("error", "The suggested timer is gone");
          return false;
        }
        const spans = [{ start: piece.start, end: piece.end }];
        await api.suggestionApproveEdited(day, piece.id, { projectId, note, tagId, spans }, "cancel");
        const project = projectLists().projects.find((p) => p.project.id === projectId)?.project.name ?? "the project";
        get().toast("success", `Timer approved for ${project}`, { label: "Undo", run: () => void get().undo() });
        return true;
      } catch (e) {
        if (toAppError(e).kind === "overlap") get().toast("error", COVERED);
        else fail(e, "Could not approve the timer");
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
