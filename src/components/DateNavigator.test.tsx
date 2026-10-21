import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, type Period, type PeriodUnit, type Range } from "../api";
import { installCommands } from "../commands";
import { addDays, addMonths } from "../lib/day";
import { defaultFilter } from "../state/slices/reports";
import { resetStore, useStore, type View } from "../state/store";
import { mondayOf } from "../test/calendarFixtures";
import { DateNavigator } from "./DateNavigator";

const mocked = vi.mocked(api, true);

/** Wed 21 Oct 2026. */
const TODAY = "2026-10-21";

/** What Rust answers for the period of `unit` around `day`. */
function period(unit: PeriodUnit, day: string): Period {
  const from = unit === "day" ? day : unit === "week" ? mondayOf(day) : addMonths(day, 0);
  const to = unit === "day" ? day : unit === "week" ? addDays(from, 6) : addDays(addMonths(day, 1), -1);
  return { from, to, holdsToday: from <= TODAY && TODAY <= to };
}

async function renderNavigator(view: View, day = TODAY, range?: Range) {
  resetStore({ view, day, ...(range && { reportFilter: { ...defaultFilter(null), range } }) });
  installCommands();
  render(<DateNavigator />);
  await screen.findByRole("group", { name: "Date" });
}

/** The heading, which opens the date picker. */
const heading = () => screen.getByTitle("Pick a date");
const expectHeading = (text: string) => vi.waitFor(() => expect(heading().textContent).toBe(text));
/** A report range called "Today" has a heading and a return button of the same name. */
const returnButton = (name: string) => {
  const found = screen.getAllByRole("button", { name }).find((b) => !b.hasAttribute("aria-haspopup"));
  if (!found) throw new Error(`No button named ${name}`);
  return found;
};
const click = (name: string) => userEvent.click(screen.getByRole("button", { name }));

describe("DateNavigator", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.calendarPeriod.mockImplementation(async (unit, day) => period(unit, day));
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(2026, 9, 21, 12));
  });

  afterEach(() => vi.useRealTimers());

  describe("labels", () => {
    it.each<[View, string, string]>([
      ["day", TODAY, "Wed, Oct 21"],
      ["day", "2026-10-05", "Mon, Oct 5"],
      ["week", TODAY, "Oct 19 – 25"],
      ["week", "2026-10-28", "Oct 26 – Nov 1"],
      ["month", TODAY, "October 2026"],
    ])("shows the %s view of %s as %s", async (view, day, label) => {
      await renderNavigator(view, day);
      await expectHeading(label);
    });

    it.each<[Range, string]>([
      [{ kind: "today" }, "Today"],
      [{ kind: "yesterday" }, "Yesterday"],
      [{ kind: "thisWeek" }, "This week"],
      [{ kind: "lastWeek" }, "Last week"],
      [{ kind: "thisMonth" }, "This month"],
      [{ kind: "lastMonth" }, "Last month"],
      [{ kind: "custom", from: "2026-10-05", to: "2026-10-09" }, "Oct 5 – 9"],
    ])("shows the report range %j as %s", async (range, label) => {
      await renderNavigator("reports", TODAY, range);
      await expectHeading(label);
    });

    it("shows the report's own period, not the shown day", async () => {
      await renderNavigator("reports", "2026-03-10");
      await expectHeading("Today");
    });

    it("shows the period the backend answers for the shown day", async () => {
      mocked.calendarPeriod.mockResolvedValue({ from: "2026-10-19", to: "2026-10-25", holdsToday: true });
      await renderNavigator("week");
      await expectHeading("Oct 19 – 25");
      expect(mocked.calendarPeriod).toHaveBeenCalledWith("week", TODAY);
    });

    it("stays hidden until the backend answers", async () => {
      mocked.calendarPeriod.mockReturnValue(new Promise(() => {}));
      resetStore({ view: "day", day: TODAY });
      render(<DateNavigator />);
      await Promise.resolve();
      expect(screen.queryByRole("group", { name: "Date" })).not.toBeInTheDocument();
    });
  });

  describe("button names", () => {
    it.each<[View, string]>([
      ["day", "day"],
      ["week", "week"],
      ["month", "month"],
      ["reports", "period"],
    ])("steps by the unit of the %s view", async (view, unit) => {
      await renderNavigator(view);
      expect(screen.getByRole("button", { name: `Previous ${unit}` })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: `Next ${unit}` })).toBeInTheDocument();
    });

    it.each<[View, string, Range | undefined]>([
      ["day", "Today", undefined],
      ["week", "This week", undefined],
      ["month", "This month", undefined],
      ["reports", "Today", { kind: "yesterday" }],
      ["reports", "This week", { kind: "lastWeek" }],
      ["reports", "This month", { kind: "lastMonth" }],
    ])("labels the return button of the %s view %s", async (view, label, range) => {
      await renderNavigator(view, "2026-01-05", range);
      expect(returnButton(label)).toBeEnabled();
    });
  });

  describe("the return button", () => {
    it.each<[boolean, string]>([
      [true, "disabled"],
      [false, "enabled"],
    ])("follows holdsToday of the period (%s: %s)", async (holdsToday, state) => {
      mocked.calendarPeriod.mockResolvedValue({ from: "2026-10-19", to: "2026-10-25", holdsToday });
      await renderNavigator("week");
      if (state === "disabled") expect(returnButton("This week")).toBeDisabled();
      else expect(returnButton("This week")).toBeEnabled();
    });

    it.each<[View, string, string]>([
      ["day", TODAY, "Today"],
      ["week", "2026-10-25", "This week"],
      ["month", "2026-10-01", "This month"],
    ])("is disabled in the %s view while it shows %s", async (view, day, label) => {
      await renderNavigator(view, day);
      expect(returnButton(label)).toBeDisabled();
    });

    it.each<[View, string, string]>([
      ["day", "2026-10-20", "Today"],
      ["week", "2026-10-26", "This week"],
      ["month", "2026-09-30", "This month"],
    ])("is enabled in the %s view while it shows %s", async (view, day, label) => {
      await renderNavigator(view, day);
      expect(returnButton(label)).toBeEnabled();
    });

    it.each<[Range, string]>([
      [{ kind: "today" }, "Today"],
      [{ kind: "thisWeek" }, "This week"],
      [{ kind: "thisMonth" }, "This month"],
    ])("is disabled while the report shows %j", async (range, label) => {
      await renderNavigator("reports", TODAY, range);
      expect(returnButton(label)).toBeDisabled();
    });

    it("is enabled for a custom report range", async () => {
      await renderNavigator("reports", TODAY, { kind: "custom", from: "2026-10-05", to: "2026-10-09" });
      expect(returnButton("Today")).toBeEnabled();
    });
  });

  describe("stepping", () => {
    it("steps the Day view by a day", async () => {
      await renderNavigator("day");
      await click("Previous day");
      await expectHeading("Tue, Oct 20");
      await click("Next day");
      await click("Next day");
      await expectHeading("Thu, Oct 22");
    });

    it("steps the Week view by a week", async () => {
      await renderNavigator("week");
      await click("Next week");
      await expectHeading("Oct 26 – Nov 1");
      await click("Previous week");
      await click("Previous week");
      await expectHeading("Oct 12 – 18");
    });

    it("steps the Month view by a month", async () => {
      await renderNavigator("month");
      await click("Previous month");
      await expectHeading("September 2026");
      await click("Next month");
      await click("Next month");
      await expectHeading("November 2026");
    });

    it("moves the report's period through the backend", async () => {
      mocked.reportRangeShift.mockResolvedValueOnce({ kind: "yesterday" });
      await renderNavigator("reports");
      await click("Previous period");
      expect(mocked.reportRangeShift).toHaveBeenCalledWith({ kind: "today" }, null, -1);
      await expectHeading("Yesterday");
    });

    it("returns the Day view to today", async () => {
      await renderNavigator("day", "2026-10-05");
      await click("Today");
      await expectHeading("Wed, Oct 21");
    });

    it("returns the Week view to this week", async () => {
      await renderNavigator("week", "2026-09-01");
      await click("This week");
      await expectHeading("Oct 19 – 25");
    });

    it("returns the Month view to this month", async () => {
      await renderNavigator("month", "2026-01-15");
      await click("This month");
      await expectHeading("October 2026");
    });

    it("returns the report to the period that holds today", async () => {
      await renderNavigator("reports", TODAY, { kind: "lastWeek" });
      await userEvent.click(returnButton("This week"));
      expect(mocked.reportRangeShift).toHaveBeenCalledWith({ kind: "lastWeek" }, TODAY, 0);
    });
  });

  describe("views without a period", () => {
    it.each<View>(["projects", "rules"])("shows no date or buttons in the %s view", (view) => {
      resetStore({ view, day: TODAY });
      installCommands();
      render(<DateNavigator />);
      expect(screen.queryByRole("group", { name: "Date" })).not.toBeInTheDocument();
      expect(screen.queryAllByRole("button")).toHaveLength(0);
    });

    it("appears when the view changes to one with a period", async () => {
      resetStore({ view: "projects", day: TODAY });
      installCommands();
      render(<DateNavigator />);
      act(() => useStore.getState().setView("day"));
      expect(await screen.findByRole("group", { name: "Date" })).toBeInTheDocument();
    });
  });
});
