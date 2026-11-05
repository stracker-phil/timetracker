import { act, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, type DayView } from "../api";
import { resetStore } from "../state/store";
import { dayView } from "../test/dayFixtures";
import { dayKey, queryData, renderHookWithClient, seedQuery } from "../test/queries";
import { useDayView, shownDayView } from "./day";

const mocked = vi.mocked(api, true);

const DAY_A = "2026-03-10";
const DAY_B = "2026-03-11";

/** A `dayView` answer the test settles by hand. */
function deferred() {
  let resolve!: (view: DayView) => void;
  const promise = new Promise<DayView>((r) => (resolve = r));
  return { promise, resolve };
}

describe("useDayView", () => {
  let day = DAY_A;

  beforeEach(() => {
    vi.clearAllMocks();
    day = DAY_A;
  });

  it("keeps the previous day as placeholder while the next day loads", async () => {
    const next = deferred();
    mocked.dayView.mockImplementation((requested) => (requested === DAY_B ? next.promise : Promise.resolve(dayView({ day: requested }))));
    const { result, rerender } = renderHookWithClient(() => useDayView(day));
    await waitFor(() => expect(result.current.data?.day).toBe(DAY_A));

    day = DAY_B;
    rerender();
    expect(result.current.data?.day).toBe(DAY_A);
    expect(result.current.isPlaceholderData).toBe(true);

    await act(async () => next.resolve(dayView({ day: DAY_B })));
    await waitFor(() => expect(result.current.data?.day).toBe(DAY_B));
    expect(result.current.isPlaceholderData).toBe(false);
  });

  it("does not show a late answer for the day that is no longer shown", async () => {
    const late = deferred();
    mocked.dayView.mockImplementation((requested) => (requested === DAY_A ? late.promise : Promise.resolve(dayView({ day: requested }))));
    const { result, rerender } = renderHookWithClient(() => useDayView(day));

    day = DAY_B;
    rerender();
    await waitFor(() => expect(result.current.data?.day).toBe(DAY_B));

    await act(async () => late.resolve(dayView({ day: DAY_A })));
    expect(result.current.data?.day).toBe(DAY_B);
    expect(queryData<DayView>(dayKey(DAY_A))?.day).toBe(DAY_A);
  });
});

describe("shownDayView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ day: DAY_A });
  });

  it("is null until the shown day has loaded", async () => {
    expect(shownDayView()).toBeNull();
    renderHookWithClient(() => useDayView(DAY_A));
    await waitFor(() => expect(shownDayView()?.day).toBe(DAY_A));
  });

  it("ignores a loaded view of another day", () => {
    seedQuery(dayKey(DAY_B), dayView({ day: DAY_B }));
    expect(shownDayView()).toBeNull();
  });

  it("follows the shown day", () => {
    seedQuery(dayKey(DAY_A), dayView({ day: DAY_A }));
    seedQuery(dayKey(DAY_B), dayView({ day: DAY_B }));
    resetStore({ day: DAY_B });
    expect(shownDayView()?.day).toBe(DAY_B);
  });
});
