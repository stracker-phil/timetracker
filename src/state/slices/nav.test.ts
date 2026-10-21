import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../../api";
import { flushUiState, resetStore, useStore } from "../store";

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

    it("step moves one calendar day in the Day view, across month ends", () => {
      resetStore({ day: "2026-03-01" });
      state().step(-1);
      expect(state().day).toBe("2026-02-28");
      state().step(1);
      state().step(1);
      expect(state().day).toBe("2026-03-02");
    });

    it.each([
      [1, "2026-03-17"],
      [-1, "2026-03-03"],
    ] as const)("step(%i) moves seven days in the Week view", (by, expected) => {
      resetStore({ day: "2026-03-10", view: "week" });
      state().step(by);
      expect(state().day).toBe(expected);
      expect(state().view).toBe("week");
    });

    it.each([
      ["2026-03-10", 1, "2026-04-01"],
      ["2026-03-10", -1, "2026-02-01"],
      ["2026-12-20", 1, "2027-01-01"],
      ["2026-01-31", -1, "2025-12-01"],
    ] as const)("step from %s by %i goes to %s in the Month view", (from, by, expected) => {
      resetStore({ day: from, view: "month" });
      state().step(by);
      expect(state().day).toBe(expected);
      expect(state().view).toBe("month");
    });

    it("remembers the day a step arrives at", () => {
      resetStore({ day: "2026-03-10", view: "week" });
      state().step(1);
      expect(state().uiState.lastDay).toBe("2026-03-17");
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

    it("setView does not change the day", () => {
      state().setView("week");
      expect(state().view).toBe("week");
      expect(state().day).toBe("2026-10-21");
    });
  });

  describe("restoreDay", () => {
    const withLastDay = (lastDay: string | null, patch = {}) => resetStore({ uiState: { ...EMPTY_UI_STATE, lastDay }, ...patch });

    it.each([
      ["yesterday", "2026-10-20"],
      ["30 days ago", "2026-09-21"],
    ])("shows the day shown last when it was %s", (_name, last) => {
      withLastDay(last);
      state().restoreDay();
      expect(state().day).toBe(last);
    });

    it.each([
      ["31 days ago", "2026-09-20"],
      ["in the future", "2026-10-25"],
      ["not remembered", null],
    ])("stays on today when the day shown last is %s", (_name, last) => {
      withLastDay(last);
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
