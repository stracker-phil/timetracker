import { focusCleared } from "../focus";
import type { Slice } from "../store";

/** The Day view's block focus; the day's view itself is a query (`useDayView`, `shownDayView`). */
export interface DaySlice {
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
  create: (set, get) => ({
    focusedBlock: null,
    detailsBlock: null,
    blockMenuRequest: 0,

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
  }),
};
