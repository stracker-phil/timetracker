import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore, useStore } from "../state/store";
import { activityRow, trackerStatus } from "../test/data";
import { ActivityDebug } from "./ActivityDebug";

const mocked = vi.mocked(api, true);

describe("ActivityDebug", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date(2026, 9, 21, 12));
    mocked.activityList.mockResolvedValue([]);
    mocked.awayList.mockResolvedValue([]);
    resetStore();
  });
  afterEach(() => vi.useRealTimers());

  it("lists the app and title of each activity", async () => {
    mocked.activityList.mockResolvedValue([
      activityRow({ id: 1, app: "Safari", title: "Start Page" }),
      activityRow({ id: 2, app: "Xcode", title: "main.rs" }),
    ]);
    render(<ActivityDebug />);
    const table = await screen.findByRole("table", { name: "Activity" });
    expect(await within(table).findByRole("cell", { name: "Safari" })).toBeInTheDocument();
    expect(within(table).getByRole("cell", { name: "Start Page" })).toBeInTheDocument();
    expect(within(table).getByRole("cell", { name: "Xcode" })).toBeInTheDocument();
    expect(within(table).getByRole("cell", { name: "main.rs" })).toBeInTheDocument();
  });

  it("lists the reason of each away span", async () => {
    mocked.awayList.mockResolvedValue([
      { id: 1, start: 0, end: 60_000, reason: "sleep" },
      { id: 2, start: 120_000, end: null, reason: "idle" },
    ]);
    render(<ActivityDebug />);
    const table = await screen.findByRole("table", { name: "Away" });
    expect(await within(table).findByRole("cell", { name: "sleep" })).toBeInTheDocument();
    expect(within(table).getByRole("cell", { name: "idle" })).toBeInTheDocument();
  });

  it("requests today by default", async () => {
    render(<ActivityDebug />);
    expect(await screen.findByLabelText("Day")).toHaveValue("2026-10-21");
    await vi.waitFor(() => expect(mocked.activityList).toHaveBeenCalledWith("2026-10-21"));
    expect(mocked.awayList).toHaveBeenCalledWith("2026-10-21");
  });

  it("requests the day before when Previous day is clicked", async () => {
    render(<ActivityDebug />);
    await userEvent.click(screen.getByRole("button", { name: "Previous day" }));
    expect(screen.getByLabelText("Day")).toHaveValue("2026-10-20");
    await vi.waitFor(() => expect(mocked.activityList).toHaveBeenLastCalledWith("2026-10-20"));
    expect(mocked.awayList).toHaveBeenLastCalledWith("2026-10-20");
  });

  it("requests the day after when Next day is clicked, across a month boundary", async () => {
    vi.setSystemTime(new Date(2026, 9, 31, 12));
    render(<ActivityDebug />);
    await userEvent.click(screen.getByRole("button", { name: "Next day" }));
    await vi.waitFor(() => expect(mocked.activityList).toHaveBeenLastCalledWith("2026-11-01"));
  });

  it("shows another day's rows after navigating", async () => {
    mocked.activityList.mockImplementation(async (day) => (day === "2026-10-20" ? [activityRow({ app: "Notes", title: "Yesterday" })] : []));
    render(<ActivityDebug />);
    await userEvent.click(screen.getByRole("button", { name: "Previous day" }));
    expect(await screen.findByRole("cell", { name: "Yesterday" })).toBeInTheDocument();
  });

  it("reloads for a tracker status only after the 10 s window since the last load", async () => {
    mocked.activityList.mockResolvedValue([activityRow({ app: "Safari", title: "Old row" })]);
    render(<ActivityDebug />);
    await screen.findByRole("cell", { name: "Old row" });

    mocked.activityList.mockResolvedValue([activityRow({ app: "Safari", title: "New row" })]);
    act(() => useStore.getState().setTracker(trackerStatus({ text: "Tracking · Safari" })));
    await vi.advanceTimersByTimeAsync(5_000);
    expect(screen.getByRole("cell", { name: "Old row" })).toBeInTheDocument();

    await vi.advanceTimersByTimeAsync(5_000);
    expect(await screen.findByRole("cell", { name: "New row" })).toBeInTheDocument();
  });

  it("shows the error message when the activity cannot be read", async () => {
    mocked.activityList.mockRejectedValue({ kind: "unavailable", message: "No data store", code: null });
    render(<ActivityDebug />);
    expect(await screen.findByText("No data store")).toBeInTheDocument();
  });
});
