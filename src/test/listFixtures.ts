// Must not import the store or "../api" at runtime: apiMock.ts imports this file while the api module is mocked.
import type { AwayPrompt, ListRow, ListView } from "../api/types";
import { at, DAY } from "./dayFixtures";

/** A list row of Safari from 09:00 to 09:30 in one span; `patch` overrides fields. */
export function listRow(patch: Partial<ListRow> = {}): ListRow {
  const firstStart = patch.firstStart ?? at("09:00");
  const lastEnd = patch.lastEnd ?? at("09:30");
  const app = patch.app ?? "Safari";
  const title = patch.title ?? "Start Page";
  const totalMs = patch.totalMs ?? lastEnd - firstStart;
  return {
    key: `tracker|${app}|${title}`,
    app,
    bundleId: "com.apple.Safari",
    label: title,
    title,
    url: null,
    path: null,
    source: "tracker",
    private: false,
    totalMs,
    gapMs: lastEnd - firstStart - totalMs,
    coveredMs: 0,
    firstStart,
    lastEnd,
    spans: [[firstStart, lastEnd]],
    blockIds: ["a1"],
    ...patch,
  };
}

/** A day's list; `totalMs` defaults to the sum of the rows. */
export function listView(patch: Partial<ListView> = {}): ListView {
  const rows = patch.rows ?? [];
  return { day: DAY, rows, totalMs: rows.reduce((sum, r) => sum + r.totalMs, 0), ...patch };
}

/** An idle away span from 12:03 to 12:45 of `DAY` (42 min), suggested snapped to 12:05 to 12:45; `patch` overrides fields. */
export function awayPrompt(patch: Partial<AwayPrompt> = {}): AwayPrompt {
  return {
    span: { id: 1, start: at("12:03"), end: at("12:45"), reason: "idle" },
    endedAt: at("12:45"),
    awayMs: 42 * 60_000,
    suggested: { start: at("12:05"), end: at("12:45") },
    ...patch,
  };
}
