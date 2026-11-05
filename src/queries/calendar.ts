import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { api, type PeriodUnit, type Range, type Shown } from "../api";

export const WEEK_KEY = ["week"] as const;
export const MONTH_KEY = ["month"] as const;
/** The sidebar's month: its days mark recordings. */
export const MINI_MONTH_KEY = ["miniMonth"] as const;
export const PICKER_KEY = ["picker"] as const;
export const PERIOD_KEY = ["period"] as const;
export const JUMPS_KEY = ["jumps"] as const;
/** The status bar's numbers of a range. */
export const PERIOD_TOTALS_KEY = ["periodTotals"] as const;

/** Every read whose labels or numbers follow the calendar and title settings. */
export const CALENDAR_KEYS = [WEEK_KEY, MONTH_KEY, MINI_MONTH_KEY, PICKER_KEY, PERIOD_KEY, JUMPS_KEY, PERIOD_TOTALS_KEY];

/** The week of `day`; the last week stays while the next loads. */
export const useWeekView = (day: string) => useQuery({ queryKey: [...WEEK_KEY, day], queryFn: () => api.weekView(day), placeholderData: keepPreviousData });

export const useMonthView = (day: string) => useQuery({ queryKey: [...MONTH_KEY, day], queryFn: () => api.monthView(day), placeholderData: keepPreviousData });

/** `today` is part of the key, so the highlight follows midnight. */
export function useMiniMonth(shownDay: string, offset: number, shown: Shown, today: string) {
  return useQuery({
    queryKey: [...MINI_MONTH_KEY, shownDay, offset, shown, today],
    queryFn: () => api.calendarMiniMonth(shownDay, offset, shown),
    placeholderData: keepPreviousData,
  });
}

export function usePickerMonth(focus: string, selected: [string, string] | null) {
  const month = focus.slice(0, 7);
  return useQuery({ queryKey: [...PICKER_KEY, month, selected], queryFn: () => api.calendarPicker(focus, selected), placeholderData: keepPreviousData });
}

/** The day, week or month holding `day`; nothing is asked while `unit` is null. */
export function usePeriod(unit: PeriodUnit | null, day: string, today: string) {
  return useQuery({
    queryKey: [...PERIOD_KEY, unit, day, today],
    queryFn: () => api.calendarPeriod(unit as PeriodUnit, day),
    enabled: unit !== null,
    placeholderData: keepPreviousData,
  });
}

/** The palette's dates for `text`; nothing is asked while it is null. */
export function useDateJumpsQuery(text: string | null) {
  return useQuery({
    queryKey: [...JUMPS_KEY, text],
    queryFn: () => api.calendarJumps(text ?? ""),
    enabled: text !== null,
    placeholderData: keepPreviousData,
    meta: { failure: "Could not look up the date" },
  });
}

/** A typed time on the local day of `on`; nothing is asked while `text` is empty. */
export function useParsedTimeQuery(text: string | null, on: number) {
  return useQuery({ queryKey: ["parseTime", text, on], queryFn: () => api.calendarParseTime(text ?? "", on), enabled: !!text });
}

export const usePeriodTotals = (range: Range) => useQuery({ queryKey: [...PERIOD_TOTALS_KEY, range], queryFn: () => api.periodTotals(range), placeholderData: keepPreviousData });
