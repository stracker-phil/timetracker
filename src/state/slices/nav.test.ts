import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../../api";
import { flushUiState, resetStore, useStore } from "../store";
import { defaultFilter } from "./reports";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();

describe("nav slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ toFake: ["Date", "setTimeout", "clearTimeout"], now: new Date(2026, 9, 21, 12) });
    resetStore();
  });

  afterEach(() => {
    flushUiState();
    vi.useRealTimers();
  });

  it("starts on today in the Day view", () => {
    expect(state().day).toBe("2026-10-21");
    expect(state().view).toBe("day");
  });

  describe("day navigation", () => {
    it("goToDay sets the day and switches to the Day view", () => {
      resetStore({ view: "reports" });
      state().goToDay("2026-10-20");
      expect(state().day).toBe("2026-10-20");
      expect(state().view).toBe("day");
    });

    it("step moves one calendar day in the Day view, across month ends", async () => {
      resetStore({ day: "2026-03-01" });
      state().step(-1);
      await vi.waitFor(() => expect(state().day).toBe("2026-02-28"));
      state().step(1);
      state().step(1);
      await vi.waitFor(() => expect(state().day).toBe("2026-03-02"));
    });

    it.each([
      [1, "2026-03-17"],
      [-1, "2026-03-03"],
    ] as const)("step(%i) moves seven days in the Week view", async (by, expected) => {
      resetStore({ day: "2026-03-10", view: "week" });
      state().step(by);
      await vi.waitFor(() => expect(state().day).toBe(expected));
      expect(state().view).toBe("week");
    });

    it.each([
      ["2026-03-10", 1, "2026-04-01"],
      ["2026-03-10", -1, "2026-02-01"],
      ["2026-12-20", 1, "2027-01-01"],
      ["2026-01-31", -1, "2025-12-01"],
    ] as const)("step from %s by %i goes to %s in the Month view", async (from, by, expected) => {
      resetStore({ day: from, view: "month" });
      state().step(by);
      await vi.waitFor(() => expect(state().day).toBe(expected));
      expect(state().view).toBe("month");
    });

    it.each(["week", "reports", "projects"] as const)("showHours shows today in the Day view from the %s view", (view) => {
      resetStore({ day: "2026-01-05", view });
      state().showHours();
      expect(state().day).toBe("2026-10-21");
      expect(state().view).toBe("day");
    });

    it("remembers the day a step arrives at", async () => {
      resetStore({ day: "2026-03-10", view: "week" });
      state().step(1);
      await vi.waitFor(() => expect(state().uiState.lastDay).toBe("2026-03-17"));
    });

    it("adds up two quick steps, each from the day the one before arrived at", async () => {
      resetStore({ day: "2026-03-10" });
      state().step(1);
      state().step(1);
      await vi.waitFor(() => expect(state().day).toBe("2026-03-12"));
    });

    it.each(["day", "week", "month"] as const)("goToday returns to today and keeps the %s view", (view) => {
      resetStore({ day: "2026-01-05", view });
      state().goToday();
      expect(state().day).toBe("2026-10-21");
      expect(state().view).toBe(view);
    });

    it("remembers the shown day and writes it after the debounce", () => {
      state().goToDay("2026-10-18");
      expect(state().uiState.lastDay).toBe("2026-10-18");
      expect(mocked.setUiState).not.toHaveBeenCalled();
      vi.advanceTimersByTime(500);
      expect(mocked.setUiState).toHaveBeenCalledWith(expect.objectContaining({ lastDay: "2026-10-18" }));
    });

    it.each(["day", "week", "month"] as const)("showDate shows the day and keeps the %s view", (view) => {
      resetStore({ day: "2026-01-05", view });
      state().showDate("2026-03-10");
      expect(state().day).toBe("2026-03-10");
      expect(state().view).toBe(view);
    });

    it("setView does not change the day", () => {
      state().setView("week");
      expect(state().view).toBe("week");
      expect(state().day).toBe("2026-10-21");
    });
  });

  describe("in the Reports view", () => {
    const thisWeek = { kind: "thisWeek" } as const;
    const lastWeek = { kind: "lastWeek" } as const;

    beforeEach(() => {
      resetStore({ view: "reports", day: "2026-03-10", reportFilter: { ...defaultFilter(null), range: thisWeek } });
    });

    it.each([-1, 1] as const)("step(%i) asks Rust to shift the report's range and shows the result", async (by) => {
      mocked.reportRangeShift.mockResolvedValue(lastWeek);
      state().step(by);
      await vi.waitFor(() => expect(state().reportFilter?.range).toEqual(lastWeek));
      expect(mocked.reportRangeShift).toHaveBeenCalledWith(thisWeek, null, by);
      expect(state().day).toBe("2026-03-10");
      expect(state().view).toBe("reports");
    });

    it("goToday asks for the period that holds today", async () => {
      mocked.reportRangeShift.mockResolvedValue({ kind: "today" });
      state().goToday();
      await vi.waitFor(() => expect(state().reportFilter?.range).toEqual({ kind: "today" }));
      expect(mocked.reportRangeShift).toHaveBeenCalledWith(thisWeek, "2026-10-21", 0);
    });

    it("showDate asks for the period that holds the day", async () => {
      const custom = { kind: "custom", from: "2026-03-10", to: "2026-03-10" } as const;
      mocked.reportRangeShift.mockResolvedValue(custom);
      state().showDate("2026-03-10");
      await vi.waitFor(() => expect(state().reportFilter?.range).toEqual(custom));
      expect(mocked.reportRangeShift).toHaveBeenCalledWith(thisWeek, "2026-03-10", 0);
    });
  });

  describe("restoreDay", () => {
    const withLastDay = (lastDay: string | null, patch = {}) => resetStore({ uiState: { ...EMPTY_UI_STATE, lastDay }, ...patch });

    it.each(["2026-10-20", "2026-09-21", "2026-01-01", "2026-10-25"])("shows whatever day Rust sent as the day shown last: %s", (last) => {
      withLastDay(last);
      state().restoreDay();
      expect(state().day).toBe(last);
    });

    it("stays on today when no day is remembered", () => {
      withLastDay(null);
      state().restoreDay();
      expect(state().day).toBe("2026-10-21");
    });

    it("keeps a day that was shown meanwhile", () => {
      resetStore();
      state().goToDay("2026-10-05");
      useStore.setState({ uiState: { ...EMPTY_UI_STATE, lastDay: "2026-10-20" } });
      state().restoreDay();
      expect(state().day).toBe("2026-10-05");
    });

    it("keeps today when it was shown meanwhile, even with a recent day remembered", () => {
      resetStore();
      state().goToDay("2026-10-21");
      useStore.setState({ uiState: { ...EMPTY_UI_STATE, lastDay: "2026-10-20" } });
      state().restoreDay();
      expect(state().day).toBe("2026-10-21");
    });
  });
});
