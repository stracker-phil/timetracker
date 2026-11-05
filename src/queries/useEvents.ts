import { useEffect } from "react";
import type { QueryKey } from "@tanstack/react-query";
import { api, type DataChanged, type DataKind } from "../api";
import { setOverrides } from "../commands/registry";
import { disposeAll } from "../lib/disposeAll";
import { useStore } from "../state/store";
import { CALENDAR_KEYS, MINI_MONTH_KEY, MONTH_KEY, PERIOD_TOTALS_KEY, WEEK_KEY } from "./calendar";
import { DAY_KEY, dayKey } from "./day";
import { PROJECT_LISTS_KEY } from "./projects";
import { invalidate } from "./queryClient";
import { REPORT_KEY } from "./reports";
import { RULE_PREVIEW_KEY, RULES_KEY, RULES_TODAY_KEY } from "./rules";
import { KEYBINDINGS_KEY, SETTINGS_KEY, setKeybindingsData, setSettingsData } from "./settings";
import { setTimerData } from "./status";
import { ACTIVITY_SOURCES_KEY, DATA_INFO_KEY } from "./system";

/** The changed day's view, or every day's when `day` is null. */
const days = (day: string | null): QueryKey[] => [day === null ? DAY_KEY : dayKey(day)];

/**
 * The query keys a `data_changed` of each kind makes stale. Recordings also change the project lists (hours,
 * last used, tag counts) and the pending suggestions of today's rules; the timeline and title settings shape
 * every day and the calendar.
 */
export const DATA_INVALIDATIONS: Record<DataKind, (day: string | null) => QueryKey[]> = {
  recordings: (day) => [...days(day), WEEK_KEY, MONTH_KEY, MINI_MONTH_KEY, PERIOD_TOTALS_KEY, REPORT_KEY, PROJECT_LISTS_KEY, RULES_TODAY_KEY, DATA_INFO_KEY],
  activity: (day) => [...days(day), WEEK_KEY, MONTH_KEY, PERIOD_TOTALS_KEY, ACTIVITY_SOURCES_KEY, RULES_TODAY_KEY, RULE_PREVIEW_KEY, DATA_INFO_KEY],
  projects: () => [PROJECT_LISTS_KEY, DAY_KEY, WEEK_KEY, MONTH_KEY, PERIOD_TOTALS_KEY, REPORT_KEY, RULES_KEY, DATA_INFO_KEY],
  tags: () => [PROJECT_LISTS_KEY, DAY_KEY, WEEK_KEY, MONTH_KEY, REPORT_KEY, RULES_KEY],
  rules: () => [RULES_KEY, RULES_TODAY_KEY, DAY_KEY],
  settings: () => [SETTINGS_KEY, KEYBINDINGS_KEY, DAY_KEY, ...CALENDAR_KEYS, REPORT_KEY],
};

export const dataChanged = ({ kind, day }: DataChanged) => invalidate(DATA_INVALIDATIONS[kind](day));

/**
 * Follows Rust while mounted: `data_changed` refetches what it names (`DATA_INVALIDATIONS`); the settings,
 * shortcuts, timer and tracker events carry their new value, which goes straight into its query.
 */
export function useEvents() {
  useEffect(() => {
    const subscriptions = [
      api.onDataChanged(dataChanged),
      api.onSettingsChanged(setSettingsData),
      api.onKeybindingsChanged((bindings) => {
        setKeybindingsData(bindings);
        setOverrides(bindings);
      }),
      api.onTimerStatus(setTimerData),
      api.onTrackerStatus((status) => useStore.getState().setTracker(status)),
    ];
    return () => disposeAll(subscriptions);
  }, []);
}
