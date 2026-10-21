import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { flushUiState, resetStore, useStore } from "../store";

const mocked = vi.mocked(api, true);

describe("view state slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    resetStore();
  });

  afterEach(() => {
    flushUiState();
    vi.useRealTimers();
  });

  describe("setDayMode", () => {
    it("shows the timeline until the list is chosen", () => {
      expect(useStore.getState().uiState.dayMode).toBe("timeline");
    });

    it("stores the mode and writes it after the debounce", () => {
      useStore.getState().setDayMode("list");
      expect(useStore.getState().uiState.dayMode).toBe("list");
      expect(mocked.setUiState).not.toHaveBeenCalled();
      vi.advanceTimersByTime(500);
      expect(mocked.setUiState).toHaveBeenCalledWith(expect.objectContaining({ dayMode: "list" }));
    });

    it("writes nothing when the mode is already shown", () => {
      useStore.getState().setDayMode("timeline");
      vi.advanceTimersByTime(500);
      expect(mocked.setUiState).not.toHaveBeenCalled();
    });
  });

  describe("setTimelineZoom", () => {
    it("stores the zoom and writes it after the debounce", () => {
      useStore.getState().setTimelineZoom(4);
      expect(useStore.getState().uiState.timelineZoom).toBe(4);
      vi.advanceTimersByTime(500);
      expect(mocked.setUiState).toHaveBeenCalledWith(expect.objectContaining({ timelineZoom: 4 }));
    });

    it.each([
      [20, 8],
      [0.2, 1],
    ])("clamps %s to %s px per minute", (asked, stored) => {
      useStore.getState().setTimelineZoom(asked);
      expect(useStore.getState().uiState.timelineZoom).toBe(stored);
    });
  });
});
