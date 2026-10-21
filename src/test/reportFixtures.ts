// Must not import the store or "../api" at runtime: apiMock.ts imports this file while the api module is mocked.
import type { ReportGroup, ReportResult, ReportRow, SavedReport } from "../api/types";
import { at, DAY } from "./dayFixtures";

/** A 09:00 to 09:45 row of `DAY` for the project Website; `patch` overrides fields. */
export function reportRow(patch: Partial<ReportRow> = {}): ReportRow {
  return {
    id: "r1",
    day: DAY,
    start: at("09:00"),
    end: at("09:45"),
    durationMs: 2_700_000,
    roundedMs: 2_700_000,
    projectId: "project-1",
    project: "Website",
    client: null,
    color: "blue",
    shade: 0,
    tags: [],
    note: "",
    ...patch,
  };
}

/** A project group of 0:45 with one row and no notes; `patch` overrides fields. */
export function reportGroup(patch: Partial<ReportGroup> = {}): ReportGroup {
  return { key: "project-1", label: "Website", color: "blue", shade: 0, durationMs: 2_700_000, roundedMs: 2_700_000, notes: [], rows: [reportRow()], ...patch };
}

/** A one-day report of `DAY` with a single 0:45 group; `patch` overrides fields. */
export function reportResult(patch: Partial<ReportResult> = {}): ReportResult {
  return {
    filter: { range: { kind: "today" }, clientIds: [], projectIds: [], tagIds: [], groupBy: "project", rounding: "none", includeArchived: false },
    from: DAY,
    to: DAY,
    groups: [reportGroup()],
    totalMs: 2_700_000,
    totalRoundedMs: 2_700_000,
    tagOverlap: false,
    ...patch,
  };
}

/** A saved report named "Weekly" for last week; `patch` overrides fields. */
export function savedReport(patch: Partial<SavedReport> = {}): SavedReport {
  return { id: "saved-1", name: "Weekly", filter: reportResult().filter, position: 0, ...patch };
}
