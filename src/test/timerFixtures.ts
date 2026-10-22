// Must not import the store or "../api" at runtime: apiMock.ts imports this file while the api module is mocked.
import type { RecentTimer, RunningTimer, TimerStatus } from "../api/types";
import { at } from "./dayFixtures";
import { recording } from "./recordingFixtures";

/** A timer running for the default project for 1:23; `patch` overrides fields. */
export function runningTimer(patch: Partial<RunningTimer> = {}): RunningTimer {
  return { recording: recording({ id: "r-timer", spans: [{ id: "r-timer-s1", start: at("09:00"), end: null }] }), projectName: "Website", color: "blue", shade: 0, elapsedMs: 83 * 60_000, ...patch };
}

/** The status with a timer running; `patch` overrides the timer's fields. */
export function runningStatus(patch: Partial<RunningTimer> = {}): TimerStatus {
  return { running: runningTimer(patch) };
}

/** A recently used project, note and tag combination; `patch` overrides fields. */
export function recentTimer(patch: Partial<RecentTimer> = {}): RecentTimer {
  return { projectId: "project-1", projectName: "Website", color: "blue", shade: 0, note: "", tagId: null, lastUsed: 0, ...patch };
}
