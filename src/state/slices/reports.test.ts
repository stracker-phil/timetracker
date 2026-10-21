import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { savedReport } from "../../test/reportFixtures";
import { resetStore, useStore } from "../store";
import { defaultFilter } from "./reports";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();
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

describe("reports slice, the opened saved report", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ savedReports: [weekly, monthly], view: "day" });
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
    expect(state().savedReports.map((r) => r.id)).toEqual(["s2"]);
  });

  it("stays when another report is deleted", async () => {
    state().openSavedReport(weekly);
    await state().deleteSavedReport("s2");
    expect(state().openedReport).toBe("s1");
  });
});
