import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type WeekView as Week } from "../../api";
import { handleKeydown, installCommands } from "../../commands";
import { resetStore, useStore } from "../../state/store";
import { dayColumn, weekView } from "../../test/calendarFixtures";
import { at, DAY, recordingBlock } from "../../test/dayFixtures";
import { projectItem, projectsStore } from "../../test/projectFixtures";
import { recording } from "../../test/recordingFixtures";
import { settings } from "../../test/settings";
import { Dialogs } from "../Dialogs";
import { Toasts } from "../Toasts";
import { WeekView } from "./WeekView";

const mocked = vi.mocked(api, true);

const HOUR_MS = 3_600_000;
const NEXT_DAY = "2026-03-11";
/** The default timeline zoom. */
const PX_PER_MINUTE = 2.5;

const website = projectItem({ project: { id: "p-site", name: "Website" } });
const standup = recordingBlock({ id: "r1", projectId: "p-site", projectName: "Website", start: at("09:00"), end: at("09:45") });

/** The week of `DAY` (Mon 9 to Sun 15) with a recording on Tue 10 and its totals. */
const recordedWeek = (extra: Partial<Week> & { today?: string } = {}) => {
  const base = weekView(extra);
  const days = base.days.map((d) => (d.day === DAY ? dayColumn({ ...d, recordings: [standup], totalMs: 45 * 60_000, trackedMs: HOUR_MS }) : d));
  return weekView({ ...extra, days, totalMs: 45 * 60_000, trackedMs: HOUR_MS });
};

function renderWeek(week: Week | null = weekView(), patch: Parameters<typeof resetStore>[0] = {}) {
  if (week) mocked.weekView.mockResolvedValue(week);
  resetStore({ view: "week", day: DAY, settings: settings(), ...projectsStore({ projects: [website] }), ...patch });
  installCommands();
  return render(
    <>
      <WeekView />
      <Toasts />
      <Dialogs />
    </>,
  );
}

const heads = () => screen.getAllByRole("columnheader");
const column = (label: string) => screen.getByLabelText(label);
const recordingEl = () => screen.getByLabelText("Website");

/** Presses the recording, moves it `dx`/`dy` px and releases it over `over` (null: over nothing). */
function drag(el: HTMLElement, over: HTMLElement | null, { dx = 120, dy = 0, altKey = false } = {}) {
  vi.mocked(document.elementFromPoint).mockReturnValue(over);
  fireEvent.pointerDown(el, { button: 0, clientX: 100, clientY: 1000 });
  fireEvent.pointerMove(el, { clientX: 100 + dx, clientY: 1000 + dy });
  fireEvent.pointerUp(el, { clientX: 100 + dx, clientY: 1000 + dy, altKey });
}

describe("WeekView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.recordingsList.mockResolvedValue([]);
    document.elementFromPoint = vi.fn();
    window.addEventListener("keydown", handleKeydown, true);
  });

  afterEach(() => {
    window.removeEventListener("keydown", handleKeydown, true);
    delete (document as { elementFromPoint?: unknown }).elementFromPoint;
  });

  describe("header", () => {
    it("shows the week's label and its recorded and tracked totals", async () => {
      renderWeek(recordedWeek());
      expect(await screen.findByRole("heading", { level: 1, name: "Week of 2026-03-09" })).toBeInTheDocument();
      expect(screen.getByLabelText("Totals")).toHaveTextContent("recorded 0:45 · tracked 1:00");
    });

    it("has no buttons to change the week", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      for (const name of ["Previous week", "Next week", "Today"]) expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    });

    it("shows the error when the week cannot be loaded", async () => {
      mocked.weekView.mockRejectedValue({ kind: "db", message: "Database locked", code: null });
      renderWeek(null);
      expect(await screen.findByText("Database locked")).toBeInTheDocument();
    });
  });

  describe("days", () => {
    it("shows seven day heads with their labels and totals", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      expect(heads().map((h) => h.textContent)).toEqual(["Mon 90:00", "Tue 100:45", "Wed 110:00", "Thu 120:00", "Fri 130:00", "Sat 140:00", "Sun 150:00"]);
    });

    it("marks today's head and column", async () => {
      renderWeek(weekView({ today: DAY }));
      await screen.findAllByRole("columnheader");
      expect(heads()[1]).toHaveClass("today");
      expect(heads()[0]).not.toHaveClass("today");
      expect(column("Tue 10")).toHaveClass("week-column--today");
      expect(column("Mon 9")).not.toHaveClass("week-column--today");
    });

    it("places the recordings of a day in its column", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      expect(within(column("Tue 10")).getByLabelText("Website")).toBeInTheDocument();
      expect(within(column("Wed 11")).queryByLabelText("Website")).not.toBeInTheDocument();
    });

    it("opens a day in the Day view when its head is clicked", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      await userEvent.click(heads()[2]);
      expect(useStore.getState().view).toBe("day");
      expect(useStore.getState().day).toBe(NEXT_DAY);
    });
  });

  describe("navigation", () => {
    it("loads the next week with N", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      await userEvent.keyboard("n");
      await vi.waitFor(() => expect(mocked.weekView).toHaveBeenLastCalledWith("2026-03-17"));
    });

    it("loads the previous week with P", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      await userEvent.keyboard("p");
      await vi.waitFor(() => expect(mocked.weekView).toHaveBeenLastCalledWith("2026-03-03"));
      expect(useStore.getState().view).toBe("week");
    });

    it("loads the week again when recordings changed", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      mocked.weekView.mockClear();
      act(() => useStore.getState().dataChanged({ kind: "recordings", day: DAY }));
      await vi.waitFor(() => expect(mocked.weekView).toHaveBeenCalledWith(DAY));
    });
  });

  describe("dropping a recording on another day", () => {
    it("moves it there and offers Undo", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      drag(recordingEl(), column("Wed 11"));
      await vi.waitFor(() => expect(mocked.recordingMove).toHaveBeenCalledWith("r1", "r1-s1", NEXT_DAY, "cancel"));
      expect(await screen.findByText("Recording moved")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
      expect(mocked.recordingUpdateSpan).not.toHaveBeenCalled();
    });

    it("moves only the dropped block's span when the recording has several", async () => {
      const second = { ...standup, spanId: "r1-s2", spanCount: 2 };
      const base = weekView();
      const days = base.days.map((d) => (d.day === DAY ? dayColumn({ ...d, recordings: [second] }) : d));
      renderWeek(weekView({ days }));
      await screen.findAllByRole("columnheader");
      drag(recordingEl(), column("Wed 11"));
      await vi.waitFor(() => expect(mocked.recordingMove).toHaveBeenCalledWith("r1", "r1-s2", NEXT_DAY, "cancel"));
    });

    it("copies it there when ⌥ is held", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      drag(recordingEl(), column("Wed 11"), { altKey: true });
      await vi.waitFor(() => expect(mocked.recordingCopy).toHaveBeenCalledWith("r1", "r1-s1", NEXT_DAY, "cancel"));
      expect(await screen.findByText("Recording copied")).toBeInTheDocument();
      expect(mocked.recordingMove).not.toHaveBeenCalled();
    });

    it("finds the column from an element inside it", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      const inner = document.createElement("span");
      column("Fri 13").append(inner);
      drag(recordingEl(), inner);
      await vi.waitFor(() => expect(mocked.recordingMove).toHaveBeenCalledWith("r1", "r1-s1", "2026-03-13", "cancel"));
    });

    it("asks before replacing on an overlap and retries with replace", async () => {
      const inTheWay = recording({ id: "r9", projectId: "p-site", start: at("09:15", NEXT_DAY), end: at("09:30", NEXT_DAY) });
      mocked.recordingMove.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps", code: null, details: [inTheWay] });
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      drag(recordingEl(), column("Wed 11"));
      const dialog = within(await screen.findByRole("dialog"));
      expect(dialog.getByText("This overlaps a recording")).toBeInTheDocument();
      await userEvent.click(dialog.getByRole("button", { name: "Replace overlapped part" }));
      await vi.waitFor(() => expect(mocked.recordingMove).toHaveBeenLastCalledWith("r1", "r1-s1", NEXT_DAY, "replace"));
    });

    it("moves it in time when released over its own day", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      drag(recordingEl(), column("Tue 10"), { dx: 0, dy: 25 });
      await vi.waitFor(() => expect(mocked.recordingUpdateSpan).toHaveBeenCalledWith("r1", "r1-s1", { start: at("09:10"), end: at("09:55") }, null, "cancel"));
      expect(mocked.recordingMove).not.toHaveBeenCalled();
      expect(mocked.recordingCopy).not.toHaveBeenCalled();
    });

    it("moves it in time when released over no day", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      drag(recordingEl(), null, { dx: 0, dy: 25 });
      await vi.waitFor(() => expect(mocked.recordingUpdateSpan).toHaveBeenCalledWith("r1", "r1-s1", { start: at("09:10"), end: at("09:55") }, null, "cancel"));
      expect(mocked.recordingMove).not.toHaveBeenCalled();
    });

    it("changes nothing when released where it was pressed", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      drag(recordingEl(), column("Wed 11"), { dx: 0 });
      expect(mocked.recordingMove).not.toHaveBeenCalled();
      expect(mocked.recordingUpdateSpan).not.toHaveBeenCalled();
    });
  });

  describe("recording editor", () => {
    it("opens a new recording an hour long, snapped down, from a double-click on an empty spot", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      fireEvent.doubleClick(column("Wed 11"), { clientY: (9 * 60 + 7) * PX_PER_MINUTE });
      const dialog = within(await screen.findByRole("dialog", { name: "New recording" }));
      expect(dialog.getByLabelText("Span 1 from")).toHaveValue("09:05");
      expect(dialog.getByLabelText("Span 1 to")).toHaveValue("10:05");
    });

    it("shows the editor in a sheet, not in a timesheet", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      fireEvent.doubleClick(column("Wed 11"), { clientY: 9 * 60 * PX_PER_MINUTE });
      const dialog = await screen.findByRole("dialog", { name: "New recording" });
      expect(within(dialog).getByRole("form", { name: "New recording" })).toBeInTheDocument();
      expect(screen.queryByRole("complementary", { name: "Timesheet" })).not.toBeInTheDocument();
    });

    it("closes the sheet with Escape without saving", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      fireEvent.doubleClick(column("Wed 11"), { clientY: 9 * 60 * PX_PER_MINUTE });
      await screen.findByRole("dialog", { name: "New recording" });
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });

    it("saves the recording of the sheet with the project and the span", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      fireEvent.doubleClick(column("Wed 11"), { clientY: 9 * 60 * PX_PER_MINUTE });
      const dialog = within(await screen.findByRole("dialog", { name: "New recording" }));
      await userEvent.type(dialog.getByRole("combobox", { name: "Project" }), "Web{Enter}");
      await userEvent.click(dialog.getByRole("button", { name: "Save" }));
      await vi.waitFor(() =>
        expect(mocked.recordingCreate).toHaveBeenCalledWith({ projectId: "p-site", spans: [{ start: at("09:00", NEXT_DAY), end: at("10:00", NEXT_DAY) }], note: "", tagId: null }, "cancel"),
      );
      await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    });

    it("only focuses a recording on a single click", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      await userEvent.click(recordingEl());
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(useStore.getState().focusedRecording).toBe("r1");
    });

    it("opens the recording that is double-clicked, found by its span", async () => {
      mocked.recordingsList.mockResolvedValue([recording({ id: "r1", projectId: "p-site", start: at("09:00"), end: at("09:45") })]);
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      await userEvent.dblClick(recordingEl());
      const dialog = within(await screen.findByRole("dialog", { name: "Edit recording" }));
      expect(await dialog.findByLabelText("Span 1 from")).toHaveValue("09:00");
      expect(mocked.recordingsList).toHaveBeenCalledWith(at("09:00"), at("09:45"));
      expect(screen.queryByRole("dialog", { name: "New recording" })).not.toBeInTheDocument();
    });
  });
});
