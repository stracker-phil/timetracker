import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { savedReport } from "../../test/reportFixtures";
import { resetStore, useStore } from "../store";
import { defaultFilter } from "./reports";

const state = () => useStore.getState();
const weekly = savedReport({ id: "s1", name: "Weekly", filter: { ...defaultFilter(null), range: { kind: "lastWeek" } } });
const monthly = savedReport({ id: "s2", name: "Monthly", filter: { ...defaultFilter(null), range: { kind: "lastMonth" } } });

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
