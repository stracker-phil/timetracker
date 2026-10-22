import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type MonthView as Month } from "../../api";
import { handleKeydown, installCommands } from "../../commands";
import { resetStore, useStore } from "../../state/store";
import { calendarEntry, monthView } from "../../test/calendarFixtures";
import { at, DAY } from "../../test/dayFixtures";
import { projectItem, projectsStore } from "../../test/projectFixtures";
import { recording } from "../../test/recordingFixtures";
import { settings } from "../../test/settings";
import { Toasts } from "../Toasts";
import { MonthView } from "./MonthView";

const mocked = vi.mocked(api, true);

const MINUTE_MS = 60_000;
const website = projectItem({ project: { id: "p-site", name: "Website" } });

const landing = calendarEntry({ id: "r1", note: "Landing page", durationMs: 90 * MINUTE_MS, start: at("09:00"), end: at("10:30") });
const docs = calendarEntry({ id: "r2", projectId: "p-docs", projectName: "Docs", color: "green", durationMs: 20 * MINUTE_MS, spanIds: ["r2-s1"] });

const busyMonth = (entries = [landing]) =>
  monthView({
    cells: {
      "2026-03-10": { recordedMs: entries.reduce((sum, e) => sum + e.durationMs, 0), entries },
      "2026-03-11": { recordedMs: 30 * MINUTE_MS, entries: [calendarEntry({ id: "r3", projectName: "Ops", durationMs: 30 * MINUTE_MS, spanIds: ["r3-s1"] })] },
    },
  });

function renderMonth(month: Month | null = monthView()) {
  if (month) mocked.monthView.mockResolvedValue(month);
  resetStore({ view: "month", day: DAY, settings: settings(), ...projectsStore({ projects: [website] }) });
  installCommands();
  return render(
    <>
      <MonthView />
      <Toasts />
    </>,
  );
}

const grid = () => screen.findByRole("grid", { name: "March 2026" });
const cell = (day: string) => screen.getByRole("gridcell", { name: day });

describe("MonthView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.recordingsList.mockResolvedValue([]);
    window.addEventListener("keydown", handleKeydown, true);
  });

  afterEach(() => window.removeEventListener("keydown", handleKeydown, true));

  describe("header", () => {
    it("shows the month's label and the recorded total", async () => {
      renderMonth(busyMonth());
      expect(await screen.findByRole("heading", { level: 1, name: "March 2026" })).toBeInTheDocument();
      expect(screen.getByLabelText("Totals")).toHaveTextContent("recorded 2h 00m");
    });

    it("has no buttons to change the month", async () => {
      renderMonth();
      await grid();
      for (const name of ["Previous month", "Next month"]) expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    });

    it("shows the error when the month cannot be loaded", async () => {
      mocked.monthView.mockRejectedValue({ kind: "db", message: "Database locked", code: null });
      renderMonth(null);
      expect(await screen.findByText("Database locked")).toBeInTheDocument();
    });
  });

  describe("grid", () => {
    it("shows rows of seven cells, the first starting on a Monday", async () => {
      renderMonth();
      const rows = within(await grid()).getAllByRole("row");
      expect(rows).toHaveLength(6);
      for (const row of rows) expect(within(row).getAllByRole("gridcell")).toHaveLength(7);
      expect(within(rows[0]).getAllByRole("gridcell")[0]).toHaveAccessibleName("2026-02-23");
      expect(within(rows[0]).getAllByRole("gridcell")[6]).toHaveAccessibleName("2026-03-01");
    });

    it("shows each cell's weekday and day of the month, also for neighboring months", async () => {
      renderMonth();
      await grid();
      expect(cell("2026-03-05")).toHaveTextContent("Thu5");
      expect(cell("2026-02-23")).toHaveTextContent("Mon23");
    });

    it("marks weekend cells only", async () => {
      renderMonth();
      await grid();
      expect(cell("2026-03-14")).toHaveClass("month-day--weekend");
      expect(cell("2026-03-15")).toHaveClass("month-day--weekend");
      expect(cell("2026-03-13")).not.toHaveClass("month-day--weekend");
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
  });

  describe("date button", () => {
    it("names the day and its recorded time", async () => {
      renderMonth(busyMonth());
      await grid();
      expect(within(cell("2026-03-10")).getByRole("button", { name: "2026-03-10, 1h 30m recorded" })).toBeInTheDocument();
      expect(within(cell("2026-03-12")).getByRole("button", { name: "2026-03-12" })).toBeInTheDocument();
    });

    it("opens the day in the Day view", async () => {
      renderMonth(busyMonth());
      await grid();
      await userEvent.click(screen.getByRole("button", { name: "2026-03-10, 1h 30m recorded" }));
      expect(useStore.getState().view).toBe("day");
      expect(useStore.getState().day).toBe("2026-03-10");
    });

    it("also opens a day of a neighboring month", async () => {
      renderMonth();
      await grid();
      await userEvent.click(screen.getByRole("button", { name: "2026-02-24" }));
      expect(useStore.getState().day).toBe("2026-02-24");
    });
  });

  describe("recording lines", () => {
    it("shows a line per recording with its note or project and its time", async () => {
      renderMonth(busyMonth([landing, docs]));
      await grid();
      const busy = within(cell("2026-03-10"));
      expect(busy.getByRole("button", { name: "Website: Landing page, 1h 30m" })).toHaveTextContent("Landing page1h 30m");
      expect(busy.getByRole("button", { name: "Docs, 20m" })).toHaveTextContent("Docs20m");
      expect(within(cell("2026-03-12")).queryByRole("button", { name: /, (\d+h )?\d+m$/ })).not.toBeInTheDocument();
    });

    it("shows every recording of a busy day", async () => {
      const many = Array.from({ length: 9 }, (_, i) => calendarEntry({ id: `m${i}`, projectName: `Project ${i}`, spanIds: [`m${i}-s1`] }));
      renderMonth(busyMonth(many));
      await grid();
      expect(within(cell("2026-03-10")).getAllByRole("button", { name: /^Project \d, 45m$/ })).toHaveLength(9);
    });

    it("opens the editor on the recording of a clicked line", async () => {
      mocked.recordingsList.mockResolvedValue([recording({ id: "r1", projectId: "p-site", note: "Landing page", start: at("09:00"), end: at("10:30") })]);
      renderMonth(busyMonth());
      await grid();
      await userEvent.click(screen.getByRole("button", { name: "Website: Landing page, 1h 30m" }));
      const dialog = within(await screen.findByRole("dialog", { name: "Edit recording" }));
      expect(await dialog.findByLabelText("Time 1 from")).toHaveValue("09:00");
      expect(await dialog.findByLabelText("Time 1 to")).toHaveValue("10:30");
    });
  });

  describe("New recording button", () => {
    it("opens the New recording sheet on Rust's default span for that day", async () => {
      mocked.recordingDefaultSpan.mockResolvedValue({ start: at("14:00"), end: at("14:30") });
      renderMonth();
      await grid();
      await userEvent.click(within(cell("2026-03-10")).getByRole("button", { name: "New recording on Tue 10" }));
      const dialog = within(await screen.findByRole("dialog", { name: "New recording" }));
      expect(mocked.recordingDefaultSpan).toHaveBeenCalledWith("2026-03-10");
      expect(await dialog.findByLabelText("Time 1 from")).toHaveValue("14:00");
    });

    it("is on every cell, also of a neighboring month", async () => {
      renderMonth();
      await grid();
      expect(screen.getAllByRole("button", { name: /^New recording on / })).toHaveLength(42);
      expect(within(cell("2026-02-23")).getByRole("button", { name: "New recording on Mon 23" })).toBeInTheDocument();
    });

    it("closes the sheet with Escape without saving", async () => {
      renderMonth();
      await grid();
      await userEvent.click(screen.getByRole("button", { name: "New recording on Tue 10" }));
      await screen.findByRole("dialog", { name: "New recording" });
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });

    it("shows an error toast and no sheet when the default span cannot be fetched", async () => {
      mocked.recordingDefaultSpan.mockRejectedValueOnce({ kind: "io", message: "no snap", code: null });
      renderMonth();
      await grid();
      await userEvent.click(screen.getByRole("button", { name: "New recording on Tue 10" }));
      expect(await screen.findByText("Could not open a new recording: no snap")).toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
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
