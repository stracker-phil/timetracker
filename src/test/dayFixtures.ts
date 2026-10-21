import type { ActivityBlock, AwayBlock, DayTotals, DayView, Gap, Lane, RecordingBlock } from "../api/types";

/** A fixed past day, so tests never depend on the clock. */
export const DAY = "2026-03-10";

/** The local midnights that start `day` and the next day. */
export function dayBounds(day: string): [number, number] {
  const [y, m, d] = day.split("-").map(Number);
  return [new Date(y, m - 1, d).getTime(), new Date(y, m - 1, d + 1).getTime()];
}

/** The local wall-clock `HH:MM` of `day` as a UTC millisecond timestamp. */
export function at(time: string, day: string = DAY): number {
  const [y, m, d] = day.split("-").map(Number);
  const [hours, minutes] = time.split(":").map(Number);
  return new Date(y, m - 1, d, hours, minutes).getTime();
}

/** A tracker block from 09:00 to 09:30 of `DAY`; `patch` overrides fields. */
export function block(patch: Partial<ActivityBlock> = {}): ActivityBlock {
  return {
    id: "a1",
    start: at("09:00"),
    end: at("09:30"),
    app: "Safari",
    bundleId: "com.apple.Safari",
    appKey: "com.apple.Safari",
    title: "Start Page",
    label: patch.title ?? "Start Page",
    url: null,
    path: null,
    source: "tracker",
    covered: false,
    members: [1],
    col: 0,
    cols: 1,
    private: false,
    ...patch,
  };
}

/** A recording of the default project from 09:00 to 09:45 of `DAY`; `patch` overrides fields. */
export function recordingBlock(patch: Partial<RecordingBlock> = {}): RecordingBlock {
  return {
    id: "r1",
    start: at("09:00"),
    end: at("09:45"),
    projectId: "project-1",
    projectName: "Website",
    clientName: null,
    color: "blue",
    shade: 0,
    note: "",
    tags: [],
    running: false,
    clippedStart: false,
    clippedEnd: false,
    ...patch,
  };
}

/** The Activity lane without blocks; `patch` overrides fields. */
export function lane(patch: Partial<Lane> = {}): Lane {
  return { id: "activity", kind: "activity", label: "Activity", appKey: null, blocks: [], ...patch };
}

/** An external source lane (`source:<name>`) whose blocks all carry that source. */
export function sourceLane(name: string, blocks: ActivityBlock[]): Lane {
  return lane({ id: `source:${name}`, kind: "source", label: name, blocks: blocks.map((b) => ({ ...b, source: `external:${name}` })) });
}

/** An app lane (`app:<app key>`). */
export function appLane(label: string, appKey: string, blocks: ActivityBlock[]): Lane {
  return lane({ id: `app:${appKey}`, kind: "app", label, appKey, blocks });
}

/** The Summary lane. */
export function summaryLane(blocks: ActivityBlock[]): Lane {
  return lane({ id: "summary", kind: "summary", label: "Summary", blocks });
}

export function away(patch: Partial<AwayBlock> = {}): AwayBlock {
  return { id: 1, start: at("12:00"), end: at("12:30"), reason: "idle", ...patch };
}

export function gap(patch: Partial<Gap> = {}): Gap {
  return { start: at("10:00"), end: at("10:10"), ...patch };
}

/**
 * A day without activity; `patch` overrides fields. `firstActivity` defaults to the earliest block start
 * (or null), `sources` to the external sources of the lanes.
 */
export function dayView(patch: Partial<DayView> = {}): DayView {
  const day = patch.day ?? DAY;
  const lanes = patch.lanes ?? [];
  const starts = lanes.flatMap((l) => l.blocks.map((b) => b.start));
  const totals: DayTotals = { recordedMs: 0, trackedMs: 0, awayMs: 0 };
  return {
    day,
    bounds: dayBounds(day),
    lanes,
    compact: false,
    hiddenApps: [],
    away: [],
    gaps: [],
    recordings: [],
    totals,
    firstActivity: starts.length > 0 ? Math.min(...starts) : null,
    sources: lanes.filter((l) => l.kind === "source").map((l) => l.label),
    ...patch,
  };
}
