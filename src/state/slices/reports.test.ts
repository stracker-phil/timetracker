import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type SavedReport } from "../../api";
import { queryData, SAVED_REPORTS_KEY, seedQuery } from "../../test/queries";
import { savedReport } from "../../test/reportFixtures";
import { resetStore, useStore } from "../store";
import { defaultFilter } from "./reports";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();
const savedIds = () => (queryData<SavedReport[]>(SAVED_REPORTS_KEY) ?? []).map((r) => r.id);
const weekly = savedReport({ id: "s1", name: "Weekly", filter: { ...defaultFilter(null), range: { kind: "lastWeek" } } });
const monthly = savedReport({ id: "s2", name: "Monthly", filter: { ...defaultFilter(null), range: { kind: "lastMonth" } } });

describe("reports slice, shifting the period", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "reports", reportFilter: { ...defaultFilter(null), range: { kind: "thisWeek" }, groupBy: "tag" } });
  });

  it("keeps the rest of the filter when the range moves", async () => {
    mocked.reportRangeShift.mockResolvedValueOnce({ kind: "lastWeek" });
    await state().shiftReportRange(null, -1);
    expect(state().reportFilter).toMatchObject({ range: { kind: "lastWeek" }, groupBy: "tag" });
  });

  it("toasts and keeps the filter when Rust rejects the shift", async () => {
    mocked.reportRangeShift.mockRejectedValueOnce({ kind: "io", message: "Out of range", code: null });
    await state().shiftReportRange(null, 1);
    expect(state().reportFilter).toMatchObject({ range: { kind: "thisWeek" }, groupBy: "tag" });
    expect(state().toasts.map((t) => t.message)).toEqual(["Could not change the report's period: Out of range"]);
  });
});

describe("reports slice, showing a period", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "reports", reportFilter: { ...defaultFilter(null), range: { kind: "thisWeek" }, groupBy: "tag" } });
  });

  it.each([
    ["day", "today"],
    ["week", "thisWeek"],
    ["month", "thisMonth"],
  ] as const)("asks Rust for the %s period holding the day, from the %s preset", async (unit, preset) => {
    mocked.reportRangeShift.mockResolvedValueOnce({ kind: "custom", from: "2026-03-23", to: "2026-03-29" });
    await state().showReportPeriod(unit, "2026-03-25");
    expect(mocked.reportRangeShift).toHaveBeenCalledWith({ kind: preset }, "2026-03-25", 0);
  });

  it("sets the returned range and keeps the rest of the filter", async () => {
    mocked.reportRangeShift.mockResolvedValueOnce({ kind: "custom", from: "2026-03-23", to: "2026-03-29" });
    await state().showReportPeriod("week", "2026-03-25");
    expect(state().reportFilter).toMatchObject({ range: { kind: "custom", from: "2026-03-23", to: "2026-03-29" }, groupBy: "tag" });
    expect(state().view).toBe("reports");
  });

  it("toasts and keeps the range when Rust rejects the period", async () => {
    mocked.reportRangeShift.mockRejectedValueOnce({ kind: "io", message: "Out of range", code: null });
    await state().showReportPeriod("month", "2026-03-25");
    expect(state().reportFilter).toMatchObject({ range: { kind: "thisWeek" }, groupBy: "tag" });
    expect(state().toasts.map((t) => t.message)).toEqual(["Could not change the report's period: Out of range"]);
  });
});

describe("reports slice, the opened saved report", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "day" });
    seedQuery(SAVED_REPORTS_KEY, [weekly, monthly]);
  });

  it("is none until a saved report is opened", () => {
    expect(state().openedReport).toBeNull();
  });

  it("is the saved report shown in the Reports view with its filter", () => {
    state().openSavedReport(weekly);
    expect(state().openedReport).toBe("s1");
    expect(state().reportFilter).toEqual(weekly.filter);
    expect(state().view).toBe("reports");
  });

  it("is forgotten when the filter changes", () => {
    state().openSavedReport(weekly);
    state().setReportFilter({ groupBy: "tag" });
    expect(state().openedReport).toBeNull();
    expect(state().reportFilter).toMatchObject({ range: { kind: "lastWeek" }, groupBy: "tag" });
  });

  it("is forgotten when that report is deleted", async () => {
    state().openSavedReport(weekly);
    await state().deleteSavedReport("s1");
    expect(state().openedReport).toBeNull();
    expect(savedIds()).toEqual(["s2"]);
  });

  it("stays when another report is deleted", async () => {
    state().openSavedReport(weekly);
    await state().deleteSavedReport("s2");
    expect(state().openedReport).toBe("s1");
  });
});

describe("reports slice, the saved reports", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ view: "reports", reportFilter: { ...defaultFilter(null), groupBy: "tag" } });
    seedQuery(SAVED_REPORTS_KEY, [weekly]);
  });

  it("saveReport sends the name and the shown filter, lists the saved report and toasts", async () => {
    mocked.savedReportSave.mockResolvedValueOnce(savedReport({ id: "s3", name: "Tags" }));
    await state().saveReport("Tags");
    expect(mocked.savedReportSave).toHaveBeenCalledWith("Tags", expect.objectContaining({ groupBy: "tag" }));
    expect(savedIds()).toEqual(["s1", "s3"]);
    expect(state().toasts.map((t) => t.message)).toEqual(['Saved report "Tags"']);
  });

  it("saveReport toasts the error and lists nothing new when saving fails", async () => {
    mocked.savedReportSave.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
    await state().saveReport("Tags");
    expect(savedIds()).toEqual(["s1"]);
    expect(state().toasts.map((t) => t.message)).toEqual(["Could not save the report: Disk full"]);
  });

  it("renameSavedReport shows the name Rust saved", async () => {
    mocked.savedReportRename.mockResolvedValueOnce(savedReport({ id: "s1", name: "Last week" }));
    await state().renameSavedReport("s1", "Last week");
    expect(mocked.savedReportRename).toHaveBeenCalledWith("s1", "Last week");
    expect(queryData<SavedReport[]>(SAVED_REPORTS_KEY)?.map((r) => r.name)).toEqual(["Last week"]);
  });

  it("deleteSavedReport keeps the report and toasts when deleting fails", async () => {
    mocked.savedReportDelete.mockRejectedValueOnce({ kind: "not_found", message: "Gone", code: null });
    await state().deleteSavedReport("s1");
    expect(savedIds()).toEqual(["s1"]);
    expect(state().toasts.map((t) => t.message)).toEqual(["Could not delete the report: Gone"]);
  });
});
