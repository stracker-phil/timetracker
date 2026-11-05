import { useQuery } from "@tanstack/react-query";
import { act, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { setTimerData } from "../../queries/status";
import { trackerStatus } from "../../test/data";
import { renderHookWithClient, seedQuery, TIMER_KEY, TRACKER_KEY } from "../../test/queries";
import { runningStatus, suggestedStatus } from "../../test/timerFixtures";
import { useLiveReload } from "./useLiveReload";

const PROBE = ["probe"] as const;

describe("useLiveReload", () => {
  let loads = 0;

  /** The shown query under `PROBE`: its value is how often it has loaded. */
  function useShown() {
    useLiveReload(true, [PROBE]);
    return useQuery({ queryKey: PROBE, queryFn: async () => ++loads }).data;
  }

  beforeEach(() => {
    loads = 0;
    vi.useFakeTimers({ shouldAdvanceTime: true });
    seedQuery(TRACKER_KEY, trackerStatus());
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("when the timer's status changes", () => {
    it("refetches the shown queries while the timer is suggested", async () => {
      seedQuery(TIMER_KEY, suggestedStatus());
      const { result } = renderHookWithClient(useShown);
      await waitFor(() => expect(result.current).toBe(1));
      act(() => void setTimerData(suggestedStatus({ note: "edited" })));
      await act(() => vi.advanceTimersByTimeAsync(10_000));
      await waitFor(() => expect(result.current).toBe(2));
    });

    it("leaves the shown queries alone while the timer is not suggested", async () => {
      seedQuery(TIMER_KEY, runningStatus());
      const { result } = renderHookWithClient(useShown);
      await waitFor(() => expect(result.current).toBe(1));
      act(() => void setTimerData(runningStatus({ note: "edited" })));
      await act(() => vi.advanceTimersByTimeAsync(11_000));
      expect(result.current).toBe(1);
    });
  });
});
