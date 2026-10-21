import type { DataInfo } from "../api/types";

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
    available: true,
    error: null,
    ...patch,
  };
}
