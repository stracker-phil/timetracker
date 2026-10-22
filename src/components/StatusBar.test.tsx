import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { EMPTY_UI_STATE } from "../api/types";
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
  });

  it("shows the snapped selection with its length", () => {
    resetStore({ selectionSnap: { start: at("09:00"), end: at("10:30") } });
    render(<StatusBar />);
    expect(screen.getByLabelText("Selection")).toHaveTextContent("09:00–10:30 · 1h 30m");
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

  describe("shown period totals", () => {
    const week = { label: "This week", recordedMs: 8 * 60 * MINUTE_MS, billableMs: (6 * 60 + 24) * MINUTE_MS, billablePercent: 80 };
    const shown = () => screen.findByLabelText("Shown period");

    beforeEach(() => {
      mocked.periodTotals.mockResolvedValue(week);
      mocked.calendarPeriod.mockResolvedValue({ from: "2026-03-09", to: "2026-03-15", holdsToday: false });
    });

    it("reads the label, the recorded time and the billable share", async () => {
      resetStore({ view: "week" });
      render(<StatusBar />);
      expect(await shown()).toHaveTextContent("This week · recorded 8h 00m · Billable 80 %");
    });

    it("shows the share as a meter with its hours", async () => {
      resetStore({ view: "week" });
      render(<StatusBar />);
      const meter = await screen.findByRole("meter", { name: "Billable" });
      expect(meter).toHaveAttribute("aria-valuenow", "80");
      expect(meter).toHaveAttribute("title", "Billable 6h 24m / 8h 00m");
    });

    it("shows no meter when Rust has no billable share", async () => {
      mocked.periodTotals.mockResolvedValue({ ...week, billableMs: 0, billablePercent: null });
      resetStore({ view: "week" });
      render(<StatusBar />);
      expect(await shown()).toHaveTextContent(/^This week · recorded 8h 00m$/);
      expect(screen.queryByRole("meter")).not.toBeInTheDocument();
    });

    it.each(["day", "week", "month"] as const)("asks for the days the %s view shows", async (view) => {
      resetStore({ view });
      render(<StatusBar />);
      await shown();
      expect(mocked.periodTotals).toHaveBeenLastCalledWith({ kind: "custom", from: "2026-03-09", to: "2026-03-15" });
    });

    it("asks for the range of the report in Reports", async () => {
      resetStore({ view: "reports", reportFilter: { ...defaultFilter(null), range: { kind: "lastWeek" } } });
      render(<StatusBar />);
      await shown();
      expect(mocked.periodTotals).toHaveBeenLastCalledWith({ kind: "lastWeek" });
    });

    it.each(["projects", "tags", "rules"] as const)("shows nothing and asks for nothing in the %s view", async (view) => {
      resetStore({ view });
      render(<StatusBar />);
      await new Promise((resolve) => setTimeout(resolve, 20));
      expect(screen.queryByLabelText("Shown period")).not.toBeInTheDocument();
      expect(mocked.periodTotals).not.toHaveBeenCalled();
    });

    it("reloads when recordings changed", async () => {
      resetStore({ view: "week" });
      render(<StatusBar />);
      await shown();
      mocked.periodTotals.mockResolvedValue({ ...week, recordedMs: 90 * MINUTE_MS, billablePercent: null });
      act(() => useStore.getState().dataChanged({ kind: "recordings", day: "2026-03-10" }));
      await vi.waitFor(() => expect(screen.getByLabelText("Shown period")).toHaveTextContent("This week · recorded 1h 30m"));
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
      expect(screen.getByRole("button", { name: "⏱ Docs 1h 23m" })).toBeInTheDocument();
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
