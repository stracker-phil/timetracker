import { addDays, daysBetween, today } from "../../lib/day";
import type { Slice } from "../store";

export type View = "day" | "week" | "month" | "reports" | "projects";

/** A remembered day older than this opens today instead. */
const RESTORE_DAYS = 30;

export interface NavSlice {
  view: View;
  /** The day the Day, Week and Month views show (`YYYY-MM-DD`). */
  day: string;
  setView: (view: View) => void;
  /** Shows `day` in the Day view. */
  goToDay: (day: string) => void;
  goToday: () => void;
  prevDay: () => void;
  nextDay: () => void;
  /** Shows the day the window showed last, when it lies within the last 30 days and no other day was shown. */
  restoreDay: () => void;
}

export const navSlice: Slice<NavSlice> = {
  create: (set, get) => {
    // Once a day was shown on request, the remembered one no longer applies.
    let shown = false;
    const show = (day: string) => {
      shown = true;
      set({ view: "day", day });
      get().rememberDay(day);
    };
    return {
      view: "day",
      day: today(),

      setView(view) {
        set({ view });
      },

      goToDay: show,
      goToday: () => show(today()),
      prevDay: () => show(addDays(get().day, -1)),
      nextDay: () => show(addDays(get().day, 1)),

      restoreDay() {
        const last = get().uiState.lastDay;
        if (!last || shown) return;
        const age = daysBetween(last, today());
        if (age >= 0 && age <= RESTORE_DAYS) set({ day: last });
      },
    };
  },
};
