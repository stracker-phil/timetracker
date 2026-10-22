import type { PeriodUnit } from "../../api";
import { today } from "../../lib/day";
import { dayStepper } from "../../lib/dayStepper";
import type { Slice } from "../store";

export type View = "day" | "week" | "month" | "reports" | "projects" | "tags" | "rules";

const STEP_UNITS: Partial<Record<View, PeriodUnit>> = { week: "week", month: "month" };

export interface NavSlice {
  view: View;
  /** The day the Day, Week and Month views show (`YYYY-MM-DD`). */
  day: string;
  /**
   * Shows `view`, then runs `then`. Leaving a view clears the selection and closes the editor, asking first
   * when it has changes; Keep editing stays, and `then` never runs.
   */
  setView: (view: View, then?: () => void) => void;
  /** Shows `day` in the Day view. */
  goToDay: (day: string) => void;
  /** Hours: today in the Day view. */
  showHours: () => void;
  /** Today, in the view shown; in Reports the report's period that holds today. */
  goToday: () => void;
  /** Shows `day` (in Reports, the period holding it) without changing the view. */
  showDate: (day: string) => void;
  /** A day, week or month back (-1) or ahead (1), by the view shown; in Reports the report's period. */
  step: (by: 1 | -1) => void;
  /** Shows the day the window showed last, unless another day was shown; Rust drops a day too old to restore. */
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
    const stepDay = dayStepper(() => get().day, stay);
    const enter = (view: View, then?: () => void) => {
      if (view === get().view) return then?.();
      get().leaveEditor(() => {
        set({ view, selection: null, selectionSnap: null });
        then?.();
      });
    };
    const show = (day: string) => enter("day", () => stay(day));
    return {
      view: "day",
      day: today(),

      setView: enter,

      goToDay: show,
      showHours: () => show(today()),
      goToday: () => get().showDate(today()),
      showDate(day) {
        if (get().view === "reports") void get().shiftReportRange(day, 0);
        else stay(day);
      },
      step(by) {
        const { view } = get();
        if (view === "reports") void get().shiftReportRange(null, by);
        else stepDay(STEP_UNITS[view] ?? "day", by);
      },

      restoreDay() {
        const last = get().uiState.lastDay;
        if (last && !shown) set({ day: last });
      },
    };
  },
};
