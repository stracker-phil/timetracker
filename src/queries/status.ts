import { queryOptions, useQuery } from "@tanstack/react-query";
import { api, type TimerStatus, type TrackerStatus } from "../api";
import { queryClient } from "./queryClient";

// The statuses arrive as events: `useEvents` writes `timer_status` here (`setTimerData`), and `tracker_status`
// through the tracker slice's `setTracker`, which also handles the permission dialog.
export const TIMER_KEY = ["timer"] as const;
export const TRACKER_KEY = ["tracker"] as const;

/** Silent when it fails: the tray and the next event still show it. */
export const timerQuery = queryOptions({ queryKey: TIMER_KEY, queryFn: api.timerStatus, gcTime: Infinity });

/** `null` while the tracker is off: without a data store `tracker_status` rejects, and Settings → Data says why. */
export const trackerQuery = queryOptions({ queryKey: TRACKER_KEY, queryFn: (): Promise<TrackerStatus | null> => api.trackerStatus().catch(() => null), gcTime: Infinity });

/** `null` until the first status arrives. */
export const useTimer = (): TimerStatus | null => useQuery(timerQuery).data ?? null;

/** `null` until loaded, and while the tracker is off. */
export const useTracker = (): TrackerStatus | null => useQuery(trackerQuery).data ?? null;

/** The tracker's status as loaded, for code outside components. */
export const currentTracker = (): TrackerStatus | null => queryClient.getQueryData(TRACKER_KEY) ?? null;

export const currentTimer = (): TimerStatus | null => queryClient.getQueryData(TIMER_KEY) ?? null;

export const setTimerData = (status: TimerStatus) => queryClient.setQueryData(TIMER_KEY, status);

/** The tracker's status as Rust asked now, past any cached one. */
export const fetchTracker = (): Promise<TrackerStatus | null> => queryClient.fetchQuery({ ...trackerQuery, staleTime: 0 });
