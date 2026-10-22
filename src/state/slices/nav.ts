import { addDays, addMonths, daysBetween, today } from "../../lib/day";
import type { Slice } from "../store";

export type View = "day" | "week" | "month" | "reports" | "projects" | "tags" | "rules";

/** A remembered day older than this opens today instead. */
const RESTORE_DAYS = 30;

export interface NavSlice {
  view: View;
  /** The day the Day, Week and Month views show (`YYYY-MM-DD`). */
  day: string;
  setView: (view: View) => void;
  /** Shows `day` in the Day view. */
  goToDay: (day: string) => void;
  /** Today, in the view shown; in Reports the report's period that holds today. */
  goToday: () => void;
  /** Shows `day` (in Reports, the period holding it) without changing the view. */
  showDate: (day: string) => void;
  /** A day, week or month back (-1) or ahead (1), by the view shown; in Reports the report's period. */
  step: (by: 1 | -1) => void;
  /** Shows the day the window showed last, when it lies within the last 30 days and no other day was shown. */
  restoreDay: () => void;
}

export const navSlice: Slice<NavSlice> = {
  create: (set, get) => {
    // Once a day was shown on request, the remembered one no longer applies.
    let shown = false;
    const stay = (day: string) => {
      shown = true;
      set({ day });
      get().rememberDay(day);
    };
    const show = (day: string) => {
      set({ view: "day" });
      stay(day);
    };
    return {
      view: "day",
      day: today(),

      setView(view) {
        set({ view });
      },

      goToDay: show,
      goToday: () => get().showDate(today()),
      showDate(day) {
        if (get().view === "reports") void get().shiftReportRange(day, 0);
        else stay(day);
      },
      step(by) {
        const { view, day } = get();
        if (view === "reports") void get().shiftReportRange(null, by);
        else if (view === "week") stay(addDays(day, 7 * by));
        else if (view === "month") stay(addMonths(day, by));
        else stay(addDays(day, by));
      },

      restoreDay() {
        const last = get().uiState.lastDay;
        if (!last || shown) return;
        const age = daysBetween(last, today());
        if (age >= 0 && age <= RESTORE_DAYS) set({ day: last });
      },
    };
  },
};
