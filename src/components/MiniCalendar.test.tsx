import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { addMonths } from "../lib/day";
import { resetStore, useStore } from "../state/store";
import { defaultFilter } from "../state/slices/reports";
import { miniMonth } from "../test/calendarFixtures";
import { DAY } from "../test/dayFixtures";
import { MiniCalendar } from "./MiniCalendar";

const mocked = vi.mocked(api, true);

const calendar = () => screen.getByRole("group", { name: "Calendar" });
const title = (name: string) => within(calendar()).findByRole("button", { name });

describe("MiniCalendar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.calendarMiniMonth.mockImplementation(async (shownDay, offset) => miniMonth({ monthStart: addMonths(shownDay, offset), selected: DAY, recorded: ["2026-03-04"] }));
    resetStore({ view: "day", day: DAY });
  });

  it("shows the month with its title, week numbers and days", async () => {
    render(<MiniCalendar />);
    expect(await title("March 2026")).toBeInTheDocument();
    expect(within(calendar()).getByRole("button", { name: "Week 11" })).toBeInTheDocument();
    expect(within(calendar()).getByRole("button", { name: "2026-03-01" })).toHaveTextContent("1");
    expect(within(calendar()).getByRole("button", { name: "2026-03-31" })).toHaveTextContent("31");
  });

  it("marks the selected day, its week and no title in the Day view", async () => {
    render(<MiniCalendar />);
    await title("March 2026");
    expect(within(calendar()).getByRole("button", { name: DAY })).toHaveAttribute("aria-current", "date");
    expect(within(calendar()).getByRole("button", { name: "2026-03-11" })).not.toHaveAttribute("aria-current");
    expect(within(calendar()).getByRole("button", { name: "Week 11" })).toHaveAttribute("aria-current", "true");
    expect(within(calendar()).getByRole("button", { name: "Week 12" })).not.toHaveAttribute("aria-current");
    expect(within(calendar()).getByRole("button", { name: "March 2026" })).not.toHaveAttribute("aria-current");
  });

  it("marks the title when the whole month is the highlighted range", async () => {
    mocked.calendarMiniMonth.mockImplementation(async (shownDay, offset) => miniMonth({ monthStart: addMonths(shownDay, offset), monthSelected: true }));
    resetStore({ view: "month", day: DAY });
    render(<MiniCalendar />);
    expect(await title("March 2026")).toHaveAttribute("aria-current", "true");
  });

  describe("clicks", () => {
    it("goes to the Day view at the clicked day", async () => {
      resetStore({ view: "projects", day: DAY });
      render(<MiniCalendar />);
      await title("March 2026");
      await userEvent.click(within(calendar()).getByRole("button", { name: "2026-03-18" }));
      expect(useStore.getState().view).toBe("day");
      expect(useStore.getState().day).toBe("2026-03-18");
    });

    it("goes to the Week view at the Monday of the clicked week", async () => {
      render(<MiniCalendar />);
      await title("March 2026");
      await userEvent.click(within(calendar()).getByRole("button", { name: "Week 13" }));
      expect(useStore.getState().view).toBe("week");
      expect(useStore.getState().day).toBe("2026-03-23");
    });

    it("goes to the Month view at the start of the month from the title", async () => {
      render(<MiniCalendar />);
      await userEvent.click(await title("March 2026"));
      expect(useStore.getState().view).toBe("month");
      expect(useStore.getState().day).toBe("2026-03-01");
    });
  });

  describe("paging", () => {
    it.each([
      ["Previous month", "February 2026", -1],
      ["Next month", "April 2026", 1],
    ])("%s shows %s without changing the shown day", async (button, name, offset) => {
      render(<MiniCalendar />);
      await title("March 2026");
      await userEvent.click(within(calendar()).getByRole("button", { name: button }));
      expect(await title(name)).toBeInTheDocument();
      expect(mocked.calendarMiniMonth).toHaveBeenLastCalledWith(DAY, offset, { kind: "period", unit: "day", day: DAY });
      expect(useStore.getState().day).toBe(DAY);
    });

    it("pages on from the paged month", async () => {
      render(<MiniCalendar />);
      await title("March 2026");
      await userEvent.click(within(calendar()).getByRole("button", { name: "Next month" }));
      await title("April 2026");
      await userEvent.click(within(calendar()).getByRole("button", { name: "Next month" }));
      expect(await title("May 2026")).toBeInTheDocument();
      expect(mocked.calendarMiniMonth).toHaveBeenLastCalledWith(DAY, 2, { kind: "period", unit: "day", day: DAY });
    });

    it("keeps the paged month while the shown day stays in its month", async () => {
      render(<MiniCalendar />);
      await title("March 2026");
      await userEvent.click(within(calendar()).getByRole("button", { name: "Next month" }));
      await title("April 2026");
      act(() => useStore.getState().goToDay("2026-03-20"));
      expect(await title("April 2026")).toBeInTheDocument();
    });

    it("returns to the shown day's month when the day moves to another month", async () => {
      render(<MiniCalendar />);
      await title("March 2026");
      await userEvent.click(within(calendar()).getByRole("button", { name: "Next month" }));
      await title("April 2026");
      act(() => useStore.getState().goToDay("2026-06-02"));
      expect(await title("June 2026")).toBeInTheDocument();
      expect(mocked.calendarMiniMonth).toHaveBeenLastCalledWith("2026-06-02", 0, { kind: "period", unit: "day", day: "2026-06-02" });
    });
  });

  describe("what it asks to highlight", () => {
    it.each(["day", "week", "month"] as const)("the shown %s in the %s view", async (view) => {
      resetStore({ view, day: DAY });
      render(<MiniCalendar />);
      await title("March 2026");
      expect(mocked.calendarMiniMonth).toHaveBeenLastCalledWith(DAY, 0, { kind: "period", unit: view, day: DAY });
    });

    it("the report's range in Reports", async () => {
      resetStore({ view: "reports", reportFilter: { ...defaultFilter(null), range: { kind: "custom", from: "2026-02-10", to: "2026-02-20" } } });
      render(<MiniCalendar />);
      await title("February 2026");
      expect(mocked.calendarMiniMonth).toHaveBeenLastCalledWith("2026-02-10", 0, { kind: "report", range: { kind: "custom", from: "2026-02-10", to: "2026-02-20" } });
    });

    it.each(["projects", "tags", "rules"] as const)("nothing in the %s view", async (view) => {
      resetStore({ view, day: DAY });
      render(<MiniCalendar />);
      await title("March 2026");
      expect(mocked.calendarMiniMonth).toHaveBeenLastCalledWith(DAY, 0, { kind: "none" });
    });
  });
});
