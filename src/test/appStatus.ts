import type { AppStatus } from "../api/types";

/** A release build with the real capture and a linked command; `patch` overrides fields. */
export function appStatus(patch: Partial<AppStatus> = {}): AppStatus {
  return {
    name: "TimeTracker",
    version: "1.4.0",
    debug: false,
    capture: "real",
    settingsFolder: "/Users/demo/Library/Application Support/com.example.timetracker",
    database: "/Users/demo/Library/Application Support/com.example.timetracker/data/timetracker.sqlite",
    logFolder: "/Users/demo/Library/Logs/com.example.timetracker",
    commandLine: { link: "installed", path: "/Users/demo/.local/bin/timetracker" },
    ...patch,
  };
}
