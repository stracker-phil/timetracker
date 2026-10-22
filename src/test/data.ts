import type { ActivityRow, DataInfo, TrackerStatus } from "../api/types";

/** An available, empty data store; `patch` overrides fields. */
export function dataInfo(patch: Partial<DataInfo> = {}): DataInfo {
  return {
    dir: "/Users/test/Library/Application Support/com.philippstracker.timetracker/data",
    file: "/Users/test/Library/Application Support/com.philippstracker.timetracker/data/timetracker.sqlite",
    sizeBytes: 0,
    activities: 0,
    recordings: 0,
    projects: 0,
    firstActivity: null,
    firstActivityLabel: null,
    available: true,
    error: null,
    binaryPath: "/Users/test/.local/bin/timetracker",
    ...patch,
  };
}

/** A running tracker without a current activity; `patch` overrides fields. */
export function trackerStatus(patch: Partial<TrackerStatus> = {}): TrackerStatus {
  return { state: { kind: "running" }, current: null, permission: "granted", text: "Tracking", ...patch };
}

/** A closed tracker row; `patch` overrides fields. */
export function activityRow(patch: Partial<ActivityRow> = {}): ActivityRow {
  return {
    id: 1,
    source: "tracker",
    start: 0,
    end: 60_000,
    app: "Safari",
    bundleId: "com.apple.Safari",
    title: "Start Page",
    url: null,
    path: null,
    private: false,
    ...patch,
  };
}
