import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { EMPTY_UI_STATE } from "../api/types";
import { today } from "../lib/day";
import { resetStore, useStore } from "../state/store";
import { defaultFilter } from "../state/slices/reports";
import { trackerStatus } from "../test/data";
import { at } from "../test/dayFixtures";
import { runningStatus } from "../test/timerFixtures";
import { StatusBar } from "./StatusBar";

const mocked = vi.mocked(api, true);
const MINUTE_MS = 60_000;

describe("StatusBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.dayTotals.mockResolvedValue({ recordedMs: 0, trackedMs: 0, awayMs: 0 });
  });

  it("shows the snapped selection with its length", () => {
    resetStore({ selectionSnap: { start: at("09:00"), end: at("10:30") } });
    render(<StatusBar />);
    expect(screen.getByLabelText("Selection")).toHaveTextContent("09:00–10:30 · 1:30");
  });

  it("shows no selection without one", () => {
    resetStore();
    render(<StatusBar />);
    expect(screen.queryByLabelText("Selection")).not.toBeInTheDocument();
  });

  it("has no zoom readout or reset button", () => {
    resetStore();
    render(<StatusBar />);
    expect(screen.queryByText(/px/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /zoom/i })).not.toBeInTheDocument();
  });

  describe("today's totals", () => {
    it("shows the recorded and tracked time of today", async () => {
      mocked.dayTotals.mockResolvedValue({ recordedMs: 60 * MINUTE_MS, trackedMs: (3 * 60 + 12) * MINUTE_MS, awayMs: 0 });
      resetStore();
      render(<StatusBar />);
      expect(await screen.findByLabelText("Today")).toHaveTextContent("recorded 1:00 · tracked 3:12");
      expect(mocked.dayTotals).toHaveBeenCalledWith(today());
    });

    it("shows them whatever day the views show", async () => {
      mocked.dayTotals.mockResolvedValue({ recordedMs: 0, trackedMs: 30 * MINUTE_MS, awayMs: 0 });
      resetStore({ day: "2026-03-10", view: "month" });
      render(<StatusBar />);
      expect(await screen.findByLabelText("Today")).toHaveTextContent("recorded 0:00 · tracked 0:30");
      expect(mocked.dayTotals).toHaveBeenCalledWith(today());
    });

    it("reloads when the data of today changed", async () => {
      resetStore();
      render(<StatusBar />);
      await screen.findByLabelText("Today");
      mocked.dayTotals.mockResolvedValue({ recordedMs: 90 * MINUTE_MS, trackedMs: 90 * MINUTE_MS, awayMs: 0 });
      act(() => useStore.getState().dataChanged({ kind: "recordings", day: today() }));
      await vi.waitFor(() => expect(screen.getByLabelText("Today")).toHaveTextContent("recorded 1:30 · tracked 1:30"));
    });

    it("keeps showing the last totals when another day changed", async () => {
      resetStore();
      render(<StatusBar />);
      await screen.findByLabelText("Today");
      mocked.dayTotals.mockResolvedValue({ recordedMs: 90 * MINUTE_MS, trackedMs: 90 * MINUTE_MS, awayMs: 0 });
      act(() => useStore.getState().dataChanged({ kind: "recordings", day: "2026-03-10" }));
      await new Promise((resolve) => setTimeout(resolve, 20));
      expect(screen.getByLabelText("Today")).toHaveTextContent("recorded 0:00 · tracked 0:00");
    });
  });

  describe("billable meter", () => {
    const eightHours = { recordedMs: 8 * 60 * MINUTE_MS, billableMs: (6 * 60 + 24) * MINUTE_MS, percent: 80 };

    beforeEach(() => {
      mocked.billableRatio.mockResolvedValue({ recordedMs: 0, billableMs: 0, percent: null });
      mocked.calendarPeriod.mockResolvedValue({ from: "2026-03-09", to: "2026-03-15", holdsToday: false });
    });

    it("shows the share with its hours as a meter", async () => {
      mocked.billableRatio.mockResolvedValue(eightHours);
      resetStore({ view: "week" });
      render(<StatusBar />);
      const meter = await screen.findByRole("meter", { name: "Billable" });
      expect(meter).toHaveTextContent("Billable 80 %");
      expect(meter).toHaveAttribute("aria-valuenow", "80");
      expect(meter).toHaveAttribute("title", "Billable 6:24 / 8:00");
    });

    it("is hidden when nothing is recorded in the period", async () => {
      resetStore({ view: "week" });
      render(<StatusBar />);
      await vi.waitFor(() => expect(mocked.billableRatio).toHaveBeenCalled());
      await screen.findByLabelText("Today");
      expect(screen.queryByRole("meter")).not.toBeInTheDocument();
    });

    it.each(["day", "week", "month"] as const)("asks for the period the %s view shows", async (view) => {
      mocked.billableRatio.mockResolvedValue(eightHours);
      resetStore({ view });
      render(<StatusBar />);
      await screen.findByRole("meter");
      expect(mocked.billableRatio).toHaveBeenLastCalledWith({ kind: "custom", from: "2026-03-09", to: "2026-03-15" });
    });

    it("asks for the range of the report in Reports", async () => {
      mocked.billableRatio.mockResolvedValue(eightHours);
      resetStore({ view: "reports", reportFilter: { ...defaultFilter(null), range: { kind: "lastWeek" } } });
      render(<StatusBar />);
      await screen.findByRole("meter");
      expect(mocked.billableRatio).toHaveBeenLastCalledWith({ kind: "lastWeek" });
    });

    it.each(["projects", "tags", "rules"] as const)("asks for today in the %s view", async (view) => {
      mocked.billableRatio.mockResolvedValue(eightHours);
      resetStore({ view });
      render(<StatusBar />);
      await screen.findByRole("meter");
      expect(mocked.billableRatio).toHaveBeenLastCalledWith({ kind: "today" });
    });
  });

  describe("tracker status", () => {
    const hidden = { uiState: { ...EMPTY_UI_STATE, sidebarHidden: true } };

    it("is left to the sidebar while the sidebar shows", () => {
      resetStore({ tracker: trackerStatus({ text: "Tracking · Mail" }) });
      render(<StatusBar />);
      expect(screen.queryByText("Tracking · Mail")).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Pause" })).not.toBeInTheDocument();
    });

    it("shows the status text with a Pause button while the sidebar is hidden", () => {
      resetStore({ ...hidden, tracker: trackerStatus({ text: "Tracking · Mail" }) });
      render(<StatusBar />);
      expect(screen.getByText("Tracking · Mail")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
    });

    it("offers Resume while paused and resumes tracking", async () => {
      mocked.trackerResume.mockResolvedValueOnce(trackerStatus({ text: "Tracking · Mail" }));
      resetStore({ ...hidden, tracker: trackerStatus({ state: { kind: "paused", until: null }, text: "Paused until resumed" }) });
      render(<StatusBar />);
      await userEvent.click(screen.getByRole("button", { name: "Resume" }));
      expect(await screen.findByText("Tracking · Mail")).toBeInTheDocument();
    });
  });

  describe("running timer", () => {
    it("shows the project and the elapsed time", () => {
      resetStore({ timer: runningStatus({ projectName: "Docs" }) });
      render(<StatusBar />);
      expect(screen.getByRole("button", { name: "⏱ Docs 1:23" })).toBeInTheDocument();
    });

    it("shows nothing without a running timer", () => {
      resetStore({ timer: { running: null } });
      render(<StatusBar />);
      expect(screen.queryByTitle("Stop timer")).not.toBeInTheDocument();
    });

    it("stops the timer when clicked", async () => {
      resetStore({ timer: runningStatus() });
      render(<StatusBar />);
      await userEvent.click(screen.getByTitle("Stop timer"));
      expect(mocked.timerStop).toHaveBeenCalled();
    });
  });
});
