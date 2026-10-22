import type { ActivityBlock, DayApp, DayTotals, DayView, Lane, RecordingBlock, TimesheetCard } from "../api/types";

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
    activeMs: (patch.end ?? at("09:30")) - (patch.start ?? at("09:00")),
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
    spanId: `${patch.id ?? "r1"}-s1`,
    spanCount: 1,
    start: at("09:00"),
    end: at("09:45"),
    projectId: "project-1",
    projectName: "Website",
    clientName: null,
    color: "blue",
    shade: 0,
    note: "",
    tag: null,
    running: false,
    clippedStart: false,
    clippedEnd: false,
    ...patch,
  };
}

/** A timesheet card of the default project for 09:00 to 09:45 of `DAY`; `patch` overrides fields. */
export function timesheetCard(patch: Partial<TimesheetCard> = {}): TimesheetCard {
  const start = patch.start ?? at("09:00");
  const end = patch.end ?? at("09:45");
  const id = patch.id ?? "r1";
  return {
    id,
    projectId: "project-1",
    projectName: "Website",
    clientName: null,
    color: "blue",
    shade: 0,
    note: "",
    tag: null,
    durationMs: end - start,
    start,
    end,
    spanIds: [`${id}-s1`],
    running: false,
    ...patch,
  };
}

/**
 * `view` with the timesheet Rust would send for its recordings: one card per recording, by its first span, spanning
 * its earliest start to its latest end. Use it for views that step through the timesheet (`]`, `[`).
 */
export function withTimesheet(view: DayView): DayView {
  const cards = new Map<string, TimesheetCard>();
  for (const r of [...view.recordings].sort((a, b) => a.start - b.start)) {
    const card = cards.get(r.id);
    if (card) {
      card.end = Math.max(card.end, r.end);
      card.durationMs += r.end - r.start;
      card.spanIds.push(r.spanId);
    } else {
      cards.set(r.id, timesheetCard({ id: r.id, projectId: r.projectId, projectName: r.projectName, clientName: r.clientName, color: r.color, shade: r.shade, note: r.note, tag: r.tag, start: r.start, end: r.end, spanIds: [r.spanId], running: r.running }));
    }
  }
  return { ...view, timesheet: [...cards.values()] };
}

/** An app a day can give a lane of its own, with 30 minutes of tracked time; `patch` overrides fields. */
export function dayApp(patch: Partial<DayApp> = {}): DayApp {
  return { appKey: "com.apple.Safari", app: "Safari", bundleId: "com.apple.Safari", activeMs: 30 * 60_000, ownLane: false, ...patch };
}

/** The Activity lane without blocks; `patch` overrides fields. */
export function lane(patch: Partial<Lane> = {}): Lane {
  return { id: "activity", kind: "activity", label: "Activity", appKey: null, bundleId: null, blocks: [], ...patch };
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

/** The snap marks Rust sends for a day: every 5 minutes from its first instant to its last, both included. */
export function dayTicks([start, end]: [number, number]): number[] {
  const step = 5 * 60_000;
  const ticks: number[] = [];
  for (let t = start; t < end; t += step) ticks.push(t);
  ticks.push(end);
  return ticks;
}

/**
 * A day without activity; `patch` overrides fields. `firstActivity` defaults to the earliest block start
 * (or null), `sources` to the external sources of the lanes, `ticks` to a 5-minute grid over the bounds.
 */
export function dayView(patch: Partial<DayView> = {}): DayView {
  const day = patch.day ?? DAY;
  const lanes = patch.lanes ?? [];
  const starts = lanes.flatMap((l) => l.blocks.map((b) => b.start));
  const totals: DayTotals = { recordedMs: 0, trackedMs: 0, awayMs: 0 };
  return {
    day,
    label: day,
    bounds: dayBounds(day),
    ticks: dayTicks(patch.bounds ?? dayBounds(day)),
    lanes,
    compact: false,
    hiddenApps: [],
    recordings: [],
    timesheet: [],
    apps: [],
    suggestions: [],
    totals,
    firstActivity: starts.length > 0 ? Math.min(...starts) : null,
    sources: lanes.filter((l) => l.kind === "source").map((l) => l.label),
    ...patch,
  };
}
