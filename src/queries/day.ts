import { keepPreviousData, queryOptions, useQuery } from "@tanstack/react-query";
import { api, type DayView } from "../api";
import { useStore } from "../state/store";
import { queryClient } from "./queryClient";

/** Every day's view; `dayKey(day)` is one day's. */
export const DAY_KEY = ["day"] as const;
export const dayKey = (day: string) => [...DAY_KEY, day] as const;

export const dayViewQuery = (day: string) => queryOptions({ queryKey: dayKey(day), queryFn: () => api.dayView(day) });

/** The day's view; while another day loads, the previous day's stays as `data` (`isPlaceholderData`). */
export const useDayView = (day: string) => useQuery({ ...dayViewQuery(day), placeholderData: keepPreviousData });

/** The shown day's view once it has loaded, for code outside components; null while it loads. */
export function shownDayView(): DayView | null {
  return queryClient.getQueryData(dayKey(useStore.getState().day)) ?? null;
}
