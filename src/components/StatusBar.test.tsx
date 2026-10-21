import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { today } from "../lib/day";
import { resetStore, useStore } from "../state/store";
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

  describe("tracker status", () => {
    it("shows the status text with a Pause button while tracking", () => {
      resetStore({ tracker: trackerStatus({ text: "Tracking · Mail" }) });
      render(<StatusBar />);
      expect(screen.getByText("Tracking · Mail")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
    });

    it("offers Resume while paused and resumes tracking", async () => {
      mocked.trackerResume.mockResolvedValueOnce(trackerStatus({ text: "Tracking · Mail" }));
      resetStore({ tracker: trackerStatus({ state: { kind: "paused", until: null }, text: "Paused until resumed" }) });
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
