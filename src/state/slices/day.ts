import { api, toAppError, type AppError, type DayView } from "../../api";
import { focusCleared } from "../focus";
import type { Slice } from "../store";

export interface DaySlice {
  /** The shown day's view; while another day loads, the previous one stays. */
  dayView: DayView | null;
  dayError: AppError | null;
  /** Loads the shown day; only the latest request's answer is applied. */
  loadDay: () => Promise<void>;
  /** The block keys act on (`ActivityBlock.id`). */
  focusedBlock: string | null;
  focusBlock: (id: string | null) => void;
  /** The block whose details card is open from the keyboard. */
  detailsBlock: string | null;
  toggleDetails: () => void;
  /** Bumped to open the focused block's context menu from the keyboard. */
  blockMenuRequest: number;
  requestBlockMenu: () => void;
}

export const daySlice: Slice<DaySlice> = {
  create: (set, get) => {
    let latest = 0;
    return {
    dayView: null,
    dayError: null,
    focusedBlock: null,
    detailsBlock: null,
    blockMenuRequest: 0,

    async loadDay() {
      const day = get().day;
      const request = ++latest;
      try {
        const view = await api.dayView(day);
        if (request !== latest) return;
        const otherDay = get().dayView?.day !== view.day;
        set({ dayView: view, dayError: null, ...(otherDay && focusCleared("day")) });
      } catch (e) {
        if (request === latest) set({ dayError: toAppError(e) });
      }
    },

    focusBlock(id) {
      set({ focusedBlock: id, detailsBlock: null, ...(id !== null && focusCleared("other")) });
    },

    toggleDetails() {
      const { focusedBlock, detailsBlock } = get();
      set({ detailsBlock: detailsBlock === null ? focusedBlock : null });
    },

    requestBlockMenu() {
      if (get().focusedBlock !== null) set((s) => ({ blockMenuRequest: s.blockMenuRequest + 1 }));
    },
    };
  },
};
