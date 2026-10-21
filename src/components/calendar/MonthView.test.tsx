import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type MonthView as Month } from "../../api";
import { handleKeydown, installCommands } from "../../commands";
import { resetStore, useStore } from "../../state/store";
import { monthView } from "../../test/calendarFixtures";
import { DAY } from "../../test/dayFixtures";
import { MonthView } from "./MonthView";

const mocked = vi.mocked(api, true);

const MINUTE_MS = 60_000;
const busyMonth = () =>
  monthView({
    cells: {
      "2026-03-10": { recordedMs: 90 * MINUTE_MS, projects: [{ projectId: "p-site", color: "blue", shade: 0, ms: 90 * MINUTE_MS }] },
      "2026-03-11": { recordedMs: 30 * MINUTE_MS },
    },
  });

function renderMonth(month: Month | null = monthView()) {
  if (month) mocked.monthView.mockResolvedValue(month);
  resetStore({ view: "month", day: DAY });
  installCommands();
  return render(<MonthView />);
}

const grid = () => screen.findByRole("grid", { name: "March 2026" });
const cell = (day: string) => screen.getByRole("button", { name: new RegExp(`^${day}`) });

describe("MonthView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.addEventListener("keydown", handleKeydown, true);
  });

  afterEach(() => window.removeEventListener("keydown", handleKeydown, true));

  describe("header", () => {
    it("shows the month's label and the recorded total", async () => {
      renderMonth(busyMonth());
      expect(await screen.findByRole("heading", { level: 1, name: "March 2026" })).toBeInTheDocument();
      expect(screen.getByLabelText("Totals")).toHaveTextContent("recorded 2:00");
    });

    it("has no buttons to change the month", async () => {
      renderMonth();
      await grid();
      for (const name of ["Previous month", "Next month", "Today"]) expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    });

    it("shows the error when the month cannot be loaded", async () => {
      mocked.monthView.mockRejectedValue({ kind: "db", message: "Database locked", code: null });
      renderMonth(null);
      expect(await screen.findByText("Database locked")).toBeInTheDocument();
    });
  });

  describe("grid", () => {
    it("shows the weekday names, Monday first, over six weeks of days", async () => {
      renderMonth();
      const days = within(await grid());
      expect(days.getAllByRole("columnheader").map((h) => h.textContent)).toEqual(["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]);
      expect(days.getAllByRole("button")).toHaveLength(42);
    });

    it("names a cell by its day and adds the recorded time when there is some", async () => {
      renderMonth(busyMonth());
      await grid();
      expect(screen.getByRole("button", { name: "2026-03-10, 1:30 recorded" })).toHaveTextContent("1:30");
      expect(screen.getByRole("button", { name: "2026-03-12" })).toHaveTextContent(/^12$/);
    });

    it("dims the days of the neighboring months", async () => {
      renderMonth();
      await grid();
      expect(cell("2026-02-23")).toHaveClass("month-day--outside");
      expect(cell("2026-04-05")).toHaveClass("month-day--outside");
      expect(cell("2026-03-01")).not.toHaveClass("month-day--outside");
      expect(cell("2026-03-31")).not.toHaveClass("month-day--outside");
    });

    it("marks today", async () => {
      renderMonth(monthView({ today: "2026-03-20" }));
      await grid();
      expect(cell("2026-03-20")).toHaveClass("month-day--today");
      expect(cell("2026-03-19")).not.toHaveClass("month-day--today");
    });

    it("opens a day in the Day view when its cell is clicked", async () => {
      renderMonth();
      await grid();
      await userEvent.click(cell("2026-02-24"));
      expect(useStore.getState().view).toBe("day");
      expect(useStore.getState().day).toBe("2026-02-24");
    });
  });

  describe("navigation", () => {
    it("loads the first of the next month with N", async () => {
      renderMonth();
      await grid();
      await userEvent.keyboard("n");
      await vi.waitFor(() => expect(mocked.monthView).toHaveBeenLastCalledWith("2026-04-01"));
      expect(useStore.getState().view).toBe("month");
    });

    it("loads the previous month with P", async () => {
      renderMonth();
      await grid();
      await userEvent.keyboard("p");
      await vi.waitFor(() => expect(mocked.monthView).toHaveBeenLastCalledWith("2026-02-01"));
    });
  });
});
