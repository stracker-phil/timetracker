import { api, type Period, type PeriodUnit, type Range, type Shown } from "../api";
import { today } from "../lib/day";
import { useApiResult } from "../lib/useApiResult";
import { useStore, type View } from "../state/store";

const CALENDAR_UNITS: Partial<Record<View, PeriodUnit>> = { day: "day", week: "week", month: "month" };
const TODAY_RANGE: Range = { kind: "today" };
const NONE: Shown = { kind: "none" };

/** The report's range, or the default one until the filter changes. */
export const useReportRange = () => useStore((s) => s.reportFilter?.range) ?? TODAY_RANGE;

/** The day, week or month the Day, Week or Month view shows (from Rust), with its unit; null elsewhere. */
export function useCalendarPeriod(): { unit: PeriodUnit; period: Period } | null {
  const view = useStore((s) => s.view);
  const day = useStore((s) => s.day);
  const unit = CALENDAR_UNITS[view];
  const { data } = useApiResult(unit ? `${unit}:${day}:${today()}` : null, () => api.calendarPeriod(unit as PeriodUnit, day), { keepData: true });
  return unit && data ? { unit, period: data } : null;
}

/** What the main view shows, for the sidebar's calendar to highlight. */
export function useShown(): Shown {
  const view = useStore((s) => s.view);
  const day = useStore((s) => s.day);
  const range = useReportRange();
  const unit = CALENDAR_UNITS[view];
  if (unit) return { kind: "period", unit, day };
  if (view === "reports") return { kind: "report", range };
  return NONE;
}

/** The day whose month the calendar opens on: the shown day, a custom report range's first day, else today. */
export function useShownDay(): string {
  const view = useStore((s) => s.view);
  const day = useStore((s) => s.day);
  const range = useReportRange();
  if (view !== "reports") return day;
  return range.kind === "custom" ? range.from : today();
}
