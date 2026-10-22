import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type CalendarEntry, type WeekView as Week } from "../../api";
import { handleKeydown, installCommands } from "../../commands";
import { resetStore, useStore } from "../../state/store";
import { calendarEntry, dayColumn, weekView } from "../../test/calendarFixtures";
import { at, DAY } from "../../test/dayFixtures";
import { projectItem, projectsStore } from "../../test/projectFixtures";
import { recording } from "../../test/recordingFixtures";
import { settings } from "../../test/settings";
import { Dialogs } from "../Dialogs";
import { Toasts } from "../Toasts";
import { WeekView } from "./WeekView";

const mocked = vi.mocked(api, true);

const HOUR_MS = 3_600_000;
const NEXT_DAY = "2026-03-11";

const website = projectItem({ project: { id: "p-site", name: "Website" } });
const standup = calendarEntry({ id: "r1", note: "standup", heightUnits: 4 });
const docs = calendarEntry({ id: "r2", projectId: "p-docs", projectName: "Docs", color: "green", start: at("13:00"), end: at("13:30"), durationMs: 30 * 60_000, spanIds: ["r2-s1"] });

/** The week of `DAY` (Mon 9 to Sun 15) with `entries` on Tue 10. */
const recordedWeek = (entries: CalendarEntry[] = [standup], extra: Partial<Week> & { today?: string } = {}) => {
  const base = weekView(extra);
  const totalMs = entries.reduce((sum, e) => sum + e.durationMs, 0);
  const days = base.days.map((d) => (d.day === DAY ? dayColumn({ ...d, entries, totalMs, trackedMs: HOUR_MS }) : d));
  return weekView({ ...extra, days, totalMs, trackedMs: HOUR_MS });
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
/** The day's column; an empty day's head carries the same label, so the head (a button) is left out. */
const column = (label: string) => screen.getAllByLabelText(label).find((el) => el.tagName !== "BUTTON") as HTMLElement;
const card = (name = "Website: standup") => screen.getByRole("button", { name });

/** Presses the card, moves it `dx` px and releases it over `over` (null: over nothing). */
function drag(el: HTMLElement, over: HTMLElement | null, { dx = 120, altKey = false } = {}) {
  vi.mocked(document.elementFromPoint).mockReturnValue(over);
  fireEvent.pointerDown(el, { button: 0, clientX: 100, clientY: 1000 });
  fireEvent.pointerMove(el, { clientX: 100 + dx, clientY: 1000 });
  fireEvent.pointerUp(el, { clientX: 100 + dx, clientY: 1000, altKey });
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
      renderWeek(recordedWeek([standup], { label: "Week 11 · Mar 9 – 15, 2026" }));
      expect(await screen.findByRole("heading", { level: 1, name: "Week 11 · Mar 9 – 15, 2026" })).toBeInTheDocument();
      expect(screen.getByLabelText("Totals")).toHaveTextContent("recorded 45m · tracked 1h 00m");
    });

    it("has no buttons to change the week", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      for (const name of ["Previous week", "Next week"]) expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    });

    it("shows the error when the week cannot be loaded", async () => {
      mocked.weekView.mockRejectedValue({ kind: "db", message: "Database locked", code: null });
      renderWeek(null);
      expect(await screen.findByText("Database locked")).toBeInTheDocument();
    });
  });

  describe("day heads", () => {
    it("shows seven heads with the weekday and date, and the recorded time only on days with recordings", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      expect(heads().map((h) => h.textContent)).toEqual(["Mon 9", "Tue 1045m", "Wed 11", "Thu 12", "Fri 13", "Sat 14", "Sun 15"]);
    });

    it("names a head by its day, adding the recorded time only when there is some", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      expect(screen.getByRole("columnheader", { name: "Mon 9" })).toBeInTheDocument();
      expect(screen.getByRole("columnheader", { name: "Tue 10, 45m recorded" })).toBeInTheDocument();
    });

    it("reads Today on today's head, which keeps its day in the label", async () => {
      renderWeek(weekView({ today: DAY }));
      await screen.findAllByRole("columnheader");
      const head = screen.getByRole("columnheader", { name: "Tue 10" });
      expect(head).toHaveTextContent("Today");
      expect(head).not.toHaveTextContent("Tue 10");
      expect(heads()[0]).toHaveTextContent("Mon 9");
    });

    it("marks the weekend heads and columns only", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      expect(heads().map((h) => h.classList.contains("weekend"))).toEqual([false, false, false, false, false, true, true]);
      expect(column("Sat 14")).toHaveClass("week-column--weekend");
      expect(column("Fri 13")).not.toHaveClass("week-column--weekend");
    });

    it("marks today's column", async () => {
      renderWeek(weekView({ today: DAY }));
      await screen.findAllByRole("columnheader");
      expect(column("Tue 10")).toHaveClass("week-column--today");
      expect(column("Mon 9")).not.toHaveClass("week-column--today");
    });

    it("opens a day in the Day view when its head is clicked", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      await userEvent.click(heads()[2]);
      expect(useStore.getState().view).toBe("day");
      expect(useStore.getState().day).toBe(NEXT_DAY);
    });
  });

  describe("cards", () => {
    it("lists a day's cards in order, then its New recording button", async () => {
      renderWeek(recordedWeek([standup, docs]));
      await screen.findAllByRole("columnheader");
      const names = within(column("Tue 10"))
        .getAllByRole("button")
        .map((b) => b.getAttribute("aria-label"));
      expect(names).toEqual(["Website: standup", "Docs", "New recording on Tue 10"]);
    });

    it("keeps a day's cards out of the other columns", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      expect(within(column("Wed 11")).queryByRole("button", { name: "Website: standup" })).not.toBeInTheDocument();
    });

    it("shows the project, note, tag, times count and duration on a card", async () => {
      renderWeek(recordedWeek([calendarEntry({ id: "r1", note: "standup", tag: "support", spanIds: ["r1-s1", "r1-s2"] })]));
      await screen.findAllByRole("columnheader");
      const el = within(card());
      for (const text of ["Website", "standup", "support", "2 times", "45m"]) expect(el.getByText(text)).toBeInTheDocument();
    });

    it.each([
      [false, false],
      [true, true],
    ])("with showNote %s the note is shown: %s", async (showNote, shown) => {
      renderWeek(recordedWeek([calendarEntry({ id: "r1", note: "standup", showNote })]));
      await screen.findAllByRole("columnheader");
      const el = within(card());
      if (shown) expect(el.getByText("standup")).toBeInTheDocument();
      else expect(el.queryByText("standup")).not.toBeInTheDocument();
    });

    it("keeps the note in a card's accessible name when the note is hidden", async () => {
      renderWeek(recordedWeek([calendarEntry({ id: "r1", note: "standup", heightUnits: 3, showNote: false })]));
      await screen.findAllByRole("columnheader");
      expect(card()).toBeInTheDocument();
    });

    it("sets a card's height through --units", async () => {
      renderWeek(recordedWeek([standup, calendarEntry({ id: "r2", heightUnits: 9, projectName: "Docs", spanIds: ["r2-s1"] })]));
      await screen.findAllByRole("columnheader");
      expect(card().style.getPropertyValue("--units")).toBe("4");
      expect(card("Docs").style.getPropertyValue("--units")).toBe("9");
    });

    it("marks only the clicked card as pressed, without opening the editor", async () => {
      renderWeek(recordedWeek([standup, docs]));
      await screen.findAllByRole("columnheader");
      await userEvent.click(card());
      expect(card()).toHaveAttribute("aria-pressed", "true");
      expect(card("Docs")).toHaveAttribute("aria-pressed", "false");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("opens the editor on a double-click", async () => {
      mocked.recordingsList.mockResolvedValue([recording({ id: "r1", projectId: "p-site", start: at("09:00"), end: at("09:45") })]);
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      await userEvent.dblClick(card());
      const dialog = within(await screen.findByRole("dialog", { name: "Edit recording" }));
      expect(await dialog.findByLabelText("Time 1 from")).toHaveValue("09:00");
    });

    it("opens the editor with ↵ on a focused card", async () => {
      mocked.recordingsList.mockResolvedValue([recording({ id: "r1", projectId: "p-site", start: at("09:00"), end: at("09:45") })]);
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      await userEvent.click(card());
      await userEvent.keyboard("{Enter}");
      expect(await screen.findByRole("dialog", { name: "Edit recording" })).toBeInTheDocument();
    });

    it("deletes a focused card with ⌫ and offers Undo", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      await userEvent.click(card());
      await userEvent.keyboard("{Backspace}");
      await vi.waitFor(() => expect(mocked.recordingDelete).toHaveBeenCalledWith("r1"));
      expect(await screen.findByText("Recording deleted")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
    });
  });

  describe("New recording button", () => {
    it("ends every column", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      for (const label of ["Mon 9", "Tue 10", "Wed 11", "Thu 12", "Fri 13", "Sat 14", "Sun 15"]) {
        const buttons = within(column(label)).getAllByRole("button");
        expect(buttons[buttons.length - 1]).toHaveAccessibleName(`New recording on ${label}`);
      }
    });

    it("is there on a day without recordings too", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      expect(within(column("Mon 9")).getAllByRole("button")).toHaveLength(1);
      expect(within(column("Mon 9")).getByRole("button", { name: "New recording on Mon 9" })).toBeInTheDocument();
    });

    it("opens a New recording sheet on Rust's default span for that day", async () => {
      mocked.recordingDefaultSpan.mockResolvedValue({ start: at("14:00", "2026-03-09"), end: at("14:30", "2026-03-09") });
      renderWeek();
      await screen.findAllByRole("columnheader");
      await userEvent.click(screen.getByRole("button", { name: "New recording on Mon 9" }));
      const dialog = within(await screen.findByRole("dialog", { name: "New recording" }));
      expect(mocked.recordingDefaultSpan).toHaveBeenCalledWith("2026-03-09");
      expect(await dialog.findByLabelText("Time 1 from")).toHaveValue("14:00");
      expect(screen.queryByRole("complementary", { name: "Timesheet" })).not.toBeInTheDocument();
    });

    it("closes the sheet with Escape without saving", async () => {
      renderWeek();
      await screen.findAllByRole("columnheader");
      await userEvent.click(screen.getByRole("button", { name: "New recording on Mon 9" }));
      await screen.findByRole("dialog", { name: "New recording" });
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });

    it("shows an error toast and no sheet when the default span cannot be fetched", async () => {
      mocked.recordingDefaultSpan.mockRejectedValueOnce({ kind: "io", message: "no snap", code: null });
      renderWeek();
      await screen.findAllByRole("columnheader");
      await userEvent.click(screen.getByRole("button", { name: "New recording on Mon 9" }));
      expect(await screen.findByText("Could not open a new recording: no snap")).toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });

  describe("dragging a card onto another column", () => {
    it("moves its time on that day there and offers Undo", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      drag(card(), column("Wed 11"));
      await vi.waitFor(() => expect(mocked.recordingMove).toHaveBeenCalledWith("r1", DAY, NEXT_DAY, "cancel"));
      expect(await screen.findByText("Recording moved")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
    });

    it("copies it there when ⌥ is held", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      drag(card(), column("Wed 11"), { altKey: true });
      await vi.waitFor(() => expect(mocked.recordingCopy).toHaveBeenCalledWith("r1", DAY, NEXT_DAY, "cancel"));
      expect(await screen.findByText("Recording copied")).toBeInTheDocument();
      expect(mocked.recordingMove).not.toHaveBeenCalled();
    });

    it("finds the column from an element inside it", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      const inner = document.createElement("span");
      column("Fri 13").append(inner);
      drag(card(), inner);
      await vi.waitFor(() => expect(mocked.recordingMove).toHaveBeenCalledWith("r1", DAY, "2026-03-13", "cancel"));
    });

    it("asks before replacing on an overlap and retries with replace", async () => {
      const inTheWay = recording({ id: "r9", projectId: "p-site", start: at("09:15", NEXT_DAY), end: at("09:30", NEXT_DAY) });
      mocked.recordingMove.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps", code: null, details: [inTheWay] });
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      drag(card(), column("Wed 11"));
      const dialog = within(await screen.findByRole("dialog"));
      expect(dialog.getByText("This overlaps a recording")).toBeInTheDocument();
      await userEvent.click(dialog.getByRole("button", { name: "Replace overlapped part" }));
      await vi.waitFor(() => expect(mocked.recordingMove).toHaveBeenLastCalledWith("r1", DAY, NEXT_DAY, "replace"));
    });

    it("does nothing when dropped on its own column", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      drag(card(), column("Tue 10"));
      expect(mocked.recordingMove).not.toHaveBeenCalled();
      expect(mocked.recordingCopy).not.toHaveBeenCalled();
      expect(screen.queryByText("Recording moved")).not.toBeInTheDocument();
    });

    it("does nothing when dropped over no column", async () => {
      renderWeek(recordedWeek());
      await screen.findAllByRole("columnheader");
      drag(card(), null);
      expect(mocked.recordingMove).not.toHaveBeenCalled();
      expect(mocked.recordingCopy).not.toHaveBeenCalled();
    });

    it("does not drag a running card", async () => {
      renderWeek(recordedWeek([calendarEntry({ id: "r1", running: true })]));
      await screen.findAllByRole("columnheader");
      drag(card("Website"), column("Wed 11"));
      expect(mocked.recordingMove).not.toHaveBeenCalled();
      expect(mocked.recordingCopy).not.toHaveBeenCalled();
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
});
