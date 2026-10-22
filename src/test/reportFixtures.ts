// Must not import the store or "../api" at runtime: apiMock.ts imports this file while the api module is mocked.
import type { ChartSlice, ReportClient,ReportProject, ReportResult, ReportRow, SavedReport } from "../api/types";
import { at, DAY } from "./dayFixtures";

/** A 09:00 to 09:45 row of `DAY` for the project Website; `patch` overrides fields. */
export function reportRow(patch: Partial<ReportRow> = {}): ReportRow {
  return {
    id: "r1",
    day: DAY,
    dayLabel: "Wed, Oct 21",
    start: at("09:00"),
    end: at("09:45"),
    spans: [{ start: patch.start ?? at("09:00"), end: patch.end ?? at("09:45") }],
    durationMs: 2_700_000,
    roundedMs: 2_700_000,
    projectId: "project-1",
    project: "Website",
    client: null,
    color: "blue",
    shade: 0,
    billable: false,
    tag: null,
    note: "",
    ...patch,
  };
}

/** A project of 45m with one row and no notes; `patch` overrides fields. */
export function reportProject(patch: Partial<ReportProject> = {}): ReportProject {
  return {
    id: "project-1",
    name: "Website",
    label: "Website",
    color: "blue",
    shade: 0,
    billable: false,
    durationMs: 2_700_000,
    roundedMs: 2_700_000,
    notes: { text: "", count: 0 },
    rows: [reportRow()],
    ...patch,
  };
}

/** A client without a name of 45m holding one project; `patch` overrides fields. */
export function reportClient(patch: Partial<ReportClient> = {}): ReportClient {
  return { key: "none", label: "No client", color: null, durationMs: 2_700_000, roundedMs: 2_700_000, projects: [reportProject()], ...patch };
}

/** A slice that takes the whole ring; `patch` overrides fields. */
export function chartSlice(patch: Partial<ChartSlice> = {}): ChartSlice {
  return { key: "project-1", label: "Website", color: "blue", shade: null, roundedMs: 2_700_000, percent: 100, offset: 0, share: 1, ...patch };
}

/** A one-day report of `DAY` with a single 45m group, client and project; `patch` overrides fields. */
export function reportResult(patch: Partial<ReportResult> = {}): ReportResult {
  return {
    filter: { range: { kind: "today" }, clientIds: [], projectIds: [], tagIds: [], groupBy: "project", rounding: "none", includeArchived: false },
    from: DAY,
    to: DAY,
    title: "Wed, Oct 21",
    totalMs: 2_700_000,
    totalRoundedMs: 2_700_000,
    billable: { recordedMs: 0, billableMs: 0, percent: null },
    recordingCount: 1,
    projectCount: 1,
    untaggedRoundedMs: 0,
    charts: { byClient: [], byProject: [], byTag: [] },
    clients: [reportClient()],
    notes: { text: "", count: 0 },
    ...patch,
  };
}

/** A saved report named "Weekly" for last week; `patch` overrides fields. */
export function savedReport(patch: Partial<SavedReport> = {}): SavedReport {
  return { id: "saved-1", name: "Weekly", filter: reportResult().filter, position: 0, ...patch };
}
