import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { api, type Span } from "../api";

// Reads with `staleTime: 0` have no event of their own and are asked again whenever a view shows them.
export const DATA_INFO_KEY = ["dataInfo"] as const;
export const ACTIVITY_SOURCES_KEY = ["activitySources"] as const;

/** The known apps (Settings → Timeline, rule conditions). */
export const useApps = () => useQuery({ queryKey: ["apps"], queryFn: api.appsList, staleTime: 0 });

/** Apps the tracker recorded in the last `days` days. */
export const useAppsSeen = (days: number) => useQuery({ queryKey: ["appsSeen", days], queryFn: () => api.appsSeen(days), staleTime: 0 });

/** Whether the LaunchAgent is installed. */
export const useAutostartStatus = () => useQuery({ queryKey: ["autostart"], queryFn: api.autostartStatus, staleTime: 0 });

/** The build and its folders (Settings → About and Status). */
export const useAppStatus = () => useQuery({ queryKey: ["appStatus"], queryFn: api.appStatus, staleTime: 0 });

/** Where the data lives and what it holds. */
export const useDataInfo = () => useQuery({ queryKey: DATA_INFO_KEY, queryFn: api.dataInfo, staleTime: 0 });

export const useActivitySources = () => useQuery({ queryKey: ACTIVITY_SOURCES_KEY, queryFn: api.activitySources, placeholderData: keepPreviousData });

/** Every menu item as the palette lists it; asked anew on each opening, since what can run changes. */
export const useMenuCommands = () => useQuery({ queryKey: ["menuCommands"], queryFn: api.menuCommands, gcTime: 0, meta: { failure: "Couldn't list the commands" } });

/** The span as Rust rolls it (an end at or before the start moves to the next day). */
export function useDraftSpan(span: Span) {
  return useQuery({ queryKey: ["draftSpan", span.start, span.end], queryFn: () => api.recordingDraftSpans([span]), placeholderData: keepPreviousData });
}
