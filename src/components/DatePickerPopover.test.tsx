import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, type PeriodUnit, type PickerMonth, type Range } from "../api";
import { installCommands, runCommand } from "../commands";
import { defaultFilter } from "../state/slices/reports";
import { resetStore, useStore, type View } from "../state/store";
import { mondayOf } from "../test/calendarFixtures";
import { MiniCalendar } from "./MiniCalendar";
import { Dialogs } from "./Dialogs";

const mocked = vi.mocked(api, true);

/** Wed 21 Oct 2026. */
const TODAY = "2026-10-21";

const dayString = (date: Date) => `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;

/** `day` moved by whole days. */
function shiftDay(day: string, by: number): string {
  const [y, m, d] = day.split("-").map(Number);
  return dayString(new Date(y, m - 1, d + by));
}

/** What Rust answers for the period of `unit` around `day`. */
function period(unit: PeriodUnit, day: string) {
  const [y, m] = day.split("-").map(Number);
  const from = unit === "day" ? day : unit === "week" ? mondayOf(day) : `${day.slice(0, 7)}-01`;
  const to = unit === "day" ? day : unit === "week" ? shiftDay(from, 6) : dayString(new Date(y, m, 0));
  return { from, to, holdsToday: from <= TODAY && TODAY <= to };
}

/** What Rust answers for the picker: six Monday-first weeks around `month`, with `selected` marked. */
function pickerMonth(month: string, selected: [string, string] | null): PickerMonth {
  const monthStart = `${month.slice(0, 7)}-01`;
  const start = mondayOf(monthStart);
  const weeks = Array.from({ length: 6 }, (_, w) =>
    Array.from({ length: 7 }, (_, d) => {
      const day = shiftDay(start, w * 7 + d);
      return { day, outside: day.slice(0, 7) !== monthStart.slice(0, 7), isToday: day === TODAY, selected: !!selected && selected[0] <= day && day <= selected[1] };
    }),
  );
  return { monthStart, weeks };
}

async function openPicker(view: View, day = TODAY, range?: Range) {
  resetStore({ view, day, ...(range && { reportFilter: { ...defaultFilter(null), range } }) });
  installCommands();
  render(
    <>
      <MiniCalendar />
      <Dialogs />
    </>,
  );
  await screen.findByRole("button", { name: /\d{4}$/ });
  await act(async () => {
    runCommand("nav.goToDate");
  });
  const picker = within(await screen.findByRole("dialog", { name: "Pick a date" }));
  await picker.findAllByRole("gridcell");
  return picker;
}

const shown = () => ({ view: useStore.getState().view, day: useStore.getState().day });

describe("DatePickerPopover", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.calendarPeriod.mockImplementation(async (unit, day) => period(unit, day));
    mocked.calendarPicker.mockImplementation(async (month, selected) => pickerMonth(month, selected));
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(2026, 9, 21, 12));
  });

  afterEach(() => vi.useRealTimers());

  it("opens from the heading on the month of the shown day, Monday first", async () => {
    const picker = await openPicker("day");
    expect(picker.getByRole("grid", { name: "October 2026" })).toBeInTheDocument();
    expect(picker.getAllByRole("columnheader").map((h) => h.textContent)).toEqual(["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]);
    expect(picker.getAllByRole("gridcell")).toHaveLength(42);
  });

  it("marks the shown day as selected and today as the current date", async () => {
    const picker = await openPicker("day", "2026-10-14");
    expect(picker.getByRole("gridcell", { name: "2026-10-14" })).toHaveAttribute("aria-selected", "true");
    expect(picker.getByRole("gridcell", { name: "2026-10-15" })).toHaveAttribute("aria-selected", "false");
    expect(picker.getByRole("gridcell", { name: TODAY })).toHaveAttribute("aria-current", "date");
  });

  it("marks every day of the shown week as selected in the Week view", async () => {
    const picker = await openPicker("week", "2026-10-14");
    const selected = picker.getAllByRole("gridcell").filter((c) => c.getAttribute("aria-selected") === "true");
    expect(selected.map((c) => c.getAttribute("aria-label"))).toEqual(["2026-10-12", "2026-10-13", "2026-10-14", "2026-10-15", "2026-10-16", "2026-10-17", "2026-10-18"]);
  });

  it("takes the flags of the cells from the backend", async () => {
    mocked.calendarPicker.mockResolvedValue({
      monthStart: "2026-10-01",
      weeks: [
        [
          { day: "2026-09-28", outside: true, isToday: false, selected: false },
          { day: "2026-09-29", outside: false, isToday: true, selected: true },
        ],
      ],
    });
    const picker = await openPicker("day");
    expect(picker.getAllByRole("gridcell")).toHaveLength(2);
    expect(picker.getByRole("gridcell", { name: "2026-09-29" })).toHaveAttribute("aria-current", "date");
    expect(picker.getByRole("gridcell", { name: "2026-09-29" })).toHaveAttribute("aria-selected", "true");
    expect(picker.getByRole("gridcell", { name: "2026-09-28" })).not.toHaveAttribute("aria-current");
    expect(picker.getByRole("gridcell", { name: "2026-09-28" })).toHaveAttribute("aria-selected", "false");
  });

  it("asks for the shown period to be marked", async () => {
    await openPicker("week", "2026-10-14");
    expect(mocked.calendarPicker).toHaveBeenLastCalledWith("2026-10-14", ["2026-10-12", "2026-10-18"]);
  });

  it("asks for nothing to be marked for a report preset", async () => {
    await openPicker("reports", TODAY, { kind: "lastWeek" });
    expect(mocked.calendarPicker).toHaveBeenLastCalledWith(TODAY, null);
  });

  describe("picking a day", () => {
    it("shows it in the Day view and closes", async () => {
      const picker = await openPicker("day");
      await userEvent.click(picker.getByRole("gridcell", { name: "2026-10-14" }));
      expect(shown()).toEqual({ view: "day", day: "2026-10-14" });
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("shows its week in the Week view", async () => {
      const picker = await openPicker("week");
      await userEvent.click(picker.getByRole("gridcell", { name: "2026-10-14" }));
      expect(shown()).toEqual({ view: "week", day: "2026-10-14" });
    });

    it("shows its month in the Month view, also for a day of a neighboring month", async () => {
      const picker = await openPicker("month");
      await userEvent.click(picker.getByRole("gridcell", { name: "2026-11-01" }));
      expect(shown()).toEqual({ view: "month", day: "2026-11-01" });
    });

    it("asks the backend for the report period that holds it", async () => {
      const picker = await openPicker("reports", TODAY, { kind: "lastWeek" });
      await userEvent.click(picker.getByRole("gridcell", { name: "2026-10-14" }));
      expect(mocked.reportRangeShift).toHaveBeenCalledWith({ kind: "lastWeek" }, "2026-10-14", 0);
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("starts a report with a custom range on its first day", async () => {
      const picker = await openPicker("reports", TODAY, { kind: "custom", from: "2026-08-03", to: "2026-08-09" });
      expect(picker.getByRole("grid", { name: "August 2026" })).toBeInTheDocument();
    });
  });

  describe("turning the month", () => {
    it("shows the previous and next month without changing the shown day", async () => {
      const picker = await openPicker("day");
      await userEvent.click(picker.getByRole("button", { name: "Previous month" }));
      expect(await picker.findByRole("grid", { name: "September 2026" })).toBeInTheDocument();
      await userEvent.click(picker.getByRole("button", { name: "Next month" }));
      await userEvent.click(picker.getByRole("button", { name: "Next month" }));
      expect(await picker.findByRole("grid", { name: "November 2026" })).toBeInTheDocument();
      expect(shown().day).toBe(TODAY);
    });

    it("steps through Rust from the month it was turned to, so quick clicks add up", async () => {
      const picker = await openPicker("day");
      await userEvent.click(picker.getByRole("button", { name: "Next month" }));
      await userEvent.click(picker.getByRole("button", { name: "Next month" }));
      await userEvent.click(picker.getByRole("button", { name: "Next month" }));
      expect(await picker.findByRole("grid", { name: "January 2027" })).toBeInTheDocument();
    });

    it("turns back across a year end", async () => {
      const picker = await openPicker("day", "2026-01-15");
      await userEvent.click(picker.getByRole("button", { name: "Previous month" }));
      expect(await picker.findByRole("grid", { name: "December 2025" })).toBeInTheDocument();
    });

    it("asks the backend for the grid of the month it was turned to", async () => {
      const picker = await openPicker("day");
      await userEvent.click(picker.getByRole("button", { name: "Previous month" }));
      await vi.waitFor(() => expect(mocked.calendarPicker).toHaveBeenLastCalledWith("2026-09-01", [TODAY, TODAY]));
    });

    it("picks a day of the month it was turned to", async () => {
      const picker = await openPicker("day");
      await userEvent.click(picker.getByRole("button", { name: "Previous month" }));
      await userEvent.click(await picker.findByRole("gridcell", { name: "2026-09-09" }));
      expect(shown().day).toBe("2026-09-09");
    });
  });

  describe("keyboard", () => {
    it("focuses the shown day, moves by arrow keys and picks with ↵", async () => {
      const picker = await openPicker("day");
      expect(picker.getByRole("gridcell", { name: TODAY })).toHaveFocus();
      await userEvent.keyboard("{ArrowDown}{ArrowLeft}");
      await vi.waitFor(() => expect(picker.getByRole("gridcell", { name: "2026-10-27" })).toHaveFocus());
      await userEvent.keyboard("{Enter}");
      expect(shown().day).toBe("2026-10-27");
    });

    it.each([
      ["{ArrowRight}", "2026-10-22"],
      ["{ArrowLeft}", "2026-10-20"],
      ["{ArrowDown}", "2026-10-28"],
      ["{ArrowUp}", "2026-10-14"],
      ["{ArrowRight}{ArrowRight}{ArrowRight}", "2026-10-24"],
      ["{ArrowUp}{ArrowUp}", "2026-10-07"],
    ])("moves the focus on %s to %s", async (keys, expected) => {
      const picker = await openPicker("day");
      await userEvent.keyboard(keys);
      await vi.waitFor(() => expect(picker.getByRole("gridcell", { name: expected })).toHaveFocus());
    });

    it("turns the month when an arrow key leaves it, and with PageDown", async () => {
      const picker = await openPicker("day", "2026-10-30");
      await userEvent.keyboard("{ArrowRight}{ArrowRight}");
      expect(await picker.findByRole("grid", { name: "November 2026" })).toBeInTheDocument();
      await userEvent.keyboard("{PageDown}");
      expect(await picker.findByRole("grid", { name: "December 2026" })).toBeInTheDocument();
    });

    it("turns to the previous month with PageUp", async () => {
      const picker = await openPicker("day");
      await userEvent.keyboard("{PageUp}");
      expect(await picker.findByRole("grid", { name: "September 2026" })).toBeInTheDocument();
      expect(shown().day).toBe(TODAY);
    });

    it("closes with Escape and keeps the shown day", async () => {
      await openPicker("day");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(shown().day).toBe(TODAY);
    });
  });
});
