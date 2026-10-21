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

    it("prevDay and nextDay step one calendar day, across month ends", () => {
      resetStore({ day: "2026-03-01" });
      state().prevDay();
      expect(state().day).toBe("2026-02-28");
      state().nextDay();
      state().nextDay();
      expect(state().day).toBe("2026-03-02");
    });

    it("goToday returns to today", () => {
      resetStore({ day: "2026-01-05", view: "month" });
      state().goToday();
      expect(state().day).toBe("2026-10-21");
      expect(state().view).toBe("day");
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
