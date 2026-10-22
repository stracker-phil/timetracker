import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type DayView } from "../../api";
import { handleKeydown, installCommands, runCommand } from "../../commands";
import { today } from "../../lib/day";
import { resetStore, useStore } from "../../state/store";
import { trackerStatus } from "../../test/data";
import { appLane, at, block, dayApp, dayBounds, DAY, dayView, lane, recordingBlock, sourceLane, summaryLane, timesheetCard, withTimesheet } from "../../test/dayFixtures";
import { projectItem, projectsStore } from "../../test/projectFixtures";
import { recording } from "../../test/recordingFixtures";
import { suggestion } from "../../test/ruleFixtures";
import { settings } from "../../test/settings";
import { Dialogs } from "../Dialogs";
import { PermissionDialog } from "../PermissionDialog";
import { Toasts } from "../Toasts";
import { DayPage } from "./DayPage";

const mocked = vi.mocked(api, true);

const VIEWPORT_PX = 600;
/** The default timeline zoom. */
const PX_PER_MINUTE = 2.5;
const MINUTE_MS = 60_000;

const docs = block({ id: "a1", app: "Safari", title: "Docs", start: at("09:00"), end: at("09:30"), url: "https://example.com/docs" });
const todo = block({ id: "a2", app: "Notes", title: "Todo", bundleId: "com.apple.Notes", appKey: "com.apple.Notes", start: at("10:00"), end: at("10:30") });
const standup = block({ id: "a3", app: "Planner", title: "Standup", bundleId: null, appKey: "Planner", start: at("10:00"), end: at("11:00") });
const busyDay = () => dayView({ lanes: [lane({ blocks: [docs, todo] }), sourceLane("planner", [standup])] });

const website = projectItem({ project: { id: "p-site", name: "Website" } });
const meeting = recordingBlock({ id: "r1", projectId: "p-site", projectName: "Website", note: "standup", start: at("11:00"), end: at("11:45") });
const meetingRow = recording({ id: "r1", projectId: "p-site", note: "standup", start: at("11:00"), end: at("11:45") });
const recordedDay = () => withTimesheet(dayView({ lanes: [lane({ blocks: [docs, todo] })], recordings: [meeting] }));
const withProject = { settings: settings(), ...projectsStore({ projects: [website] }) };

/** The `clientY` of a wall-clock time of `DAY` on the unscrolled timeline. */
const px = (time: string) => ((at(time) - dayBounds(DAY)[0]) / MINUTE_MS) * PX_PER_MINUTE;

function renderDay(view: DayView, patch: Parameters<typeof resetStore>[0] = {}) {
  mocked.dayView.mockResolvedValue(view);
  resetStore({ day: view.day, permissionDialogHidden: true, settings: settings(), ...patch });
  installCommands();
  return render(
    <>
      <DayPage />
      <Toasts />
      <PermissionDialog />
      <Dialogs />
    </>,
  );
}

const blockEl = (name: string) => screen.findByLabelText(name);
const editorForm = (name: "New recording" | "Edit recording" | "Edit suggestion") => screen.findByRole("form", { name });
const sheetOf = () => screen.getByRole("complementary", { name: "Timesheet" });
/** The From and To of the editor's `row`th span row (1-based). */
function expectSpan(form: HTMLElement, from: string, to: string, row = 1) {
  expect(within(form).getByLabelText(`Time ${row} from`)).toHaveValue(from);
  expect(within(form).getByLabelText(`Time ${row} to`)).toHaveValue(to);
}
const scrollBox = () => screen.getByRole("row").parentElement as HTMLElement;
/** The element that takes the press of a selection drag: the parent of the lanes. */
const lanesEl = () => screen.getByLabelText("Safari: Docs").closest("[data-lane-id]")?.parentElement as HTMLElement;

describe("DayPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.recordingSnap.mockImplementation(async (start, end) => ({ start, end }));
    mocked.recordingsList.mockResolvedValue([]);
    // jsdom has no layout: only the scroll box gets a height, so the visible range is meaningful.
    Object.defineProperty(HTMLElement.prototype, "clientHeight", {
      configurable: true,
      get(this: HTMLElement) {
        return this.firstElementChild?.getAttribute("role") === "row" ? VIEWPORT_PX : 0;
      },
    });
    window.addEventListener("keydown", handleKeydown, true);
  });

  afterEach(() => {
    window.removeEventListener("keydown", handleKeydown, true);
    delete (HTMLElement.prototype as { clientHeight?: number }).clientHeight;
    vi.useRealTimers();
  });

  describe("blocks", () => {
    it("renders one element per block under the labels of its lanes", async () => {
      renderDay(busyDay());
      expect(await blockEl("Safari: Docs")).toBeInTheDocument();
      expect(screen.getByLabelText("Notes: Todo")).toBeInTheDocument();
      expect(screen.getByLabelText("Planner: Standup")).toBeInTheDocument();
      const heads = screen.getAllByRole("columnheader");
      expect(heads).toHaveLength(2);
      expect(heads[0]).toHaveTextContent("Activity");
      expect(heads[1]).toHaveTextContent("planner");
    });

    it("shows the tracked time of the day in the head of the activity lane", async () => {
      const totals = { recordedMs: 0, trackedMs: (6 * 60 + 12) * MINUTE_MS, awayMs: 0 };
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], totals }));
      await blockEl("Safari: Docs");
      expect(within(screen.getByRole("columnheader")).getByText("6h 12m")).toBeInTheDocument();
    });

    it("leaves out blocks outside the visible range and renders them once scrolled to", async () => {
      const early = block({ id: "a8", app: "Mail", title: "Early", start: at("01:00"), end: at("01:30") });
      const late = block({ id: "a9", app: "Mail", title: "Late", start: at("22:00"), end: at("22:30") });
      renderDay(dayView({ lanes: [lane({ blocks: [early, docs, late] })], firstActivity: docs.start }));
      await blockEl("Safari: Docs");
      expect(screen.queryByLabelText("Mail: Early")).not.toBeInTheDocument();
      expect(screen.queryByLabelText("Mail: Late")).not.toBeInTheDocument();

      Object.defineProperty(scrollBox(), "scrollTop", { configurable: true, writable: true, value: 3000 });
      fireEvent.scroll(scrollBox());
      expect(await blockEl("Mail: Late")).toBeInTheDocument();
      expect(screen.queryByLabelText("Safari: Docs")).not.toBeInTheDocument();
    });

    it("sizes a block by the timeline zoom", async () => {
      renderDay(busyDay(), { uiState: { ...useStore.getState().uiState, timelineZoom: 4 } });
      expect(await blockEl("Safari: Docs")).toHaveStyle({ height: "120px" });
    });

    it("shows a private block as Private", async () => {
      const secret = block({ id: "a5", app: "Safari", title: "Private", private: true });
      renderDay(dayView({ lanes: [lane({ blocks: [secret] })] }));
      expect(await blockEl("Safari: Private")).toHaveTextContent("Private");
    });

    it("shows no away text on a day with activity", async () => {
      const totals = { recordedMs: 0, trackedMs: 30 * MINUTE_MS, awayMs: 30 * MINUTE_MS };
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], totals }));
      await blockEl("Safari: Docs");
      expect(screen.queryByText(/^Away ·/)).not.toBeInTheDocument();
    });
  });

  describe("header", () => {
    it("titles the toolbar with the label of the day view", async () => {
      renderDay(dayView({ label: "Tuesday, March 10", lanes: [lane({ blocks: [docs] })] }));
      expect(await screen.findByRole("heading", { level: 1, name: "Tuesday, March 10" })).toBeInTheDocument();
    });

    it("shows the totals of the day", async () => {
      const totals = { recordedMs: 0, trackedMs: (6 * 60 + 12) * 60_000, awayMs: (60 + 5) * 60_000 };
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], totals }));
      expect(await screen.findByText("tracked 6h 12m · away 1h 05m")).toBeInTheDocument();
    });

    it("shows the recorded time of the day and leaves out the zero parts", async () => {
      const totals = { recordedMs: 90 * MINUTE_MS, trackedMs: 0, awayMs: 0 };
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], recordings: [meeting], totals }));
      expect(await screen.findByLabelText("Totals")).toHaveTextContent(/^recorded 1h 30m$/);
    });

    it("has no buttons to change the day", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      for (const name of ["Previous day", "Next day", "Today"]) expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    });

    it("shows the totals on today too", async () => {
      const totals = { recordedMs: 0, trackedMs: 60 * MINUTE_MS, awayMs: 0 };
      renderDay(dayView({ day: today(), totals }));
      expect(await screen.findByLabelText("Totals")).toHaveTextContent("tracked 1h 00m");
    });

    it("shows no totals on a day with everything at zero", async () => {
      renderDay(dayView());
      await screen.findByText("Nothing tracked on this day");
      expect(screen.queryByLabelText("Totals")).not.toBeInTheDocument();
    });
  });

  describe("block labels", () => {
    it("shows the label of a block and names its app only in the accessible name", async () => {
      const doc = block({ id: "a6", app: "Preview", title: "Report.pdf — Page 2", label: "Report.pdf" });
      renderDay(dayView({ lanes: [lane({ blocks: [doc] })] }));
      const el = await blockEl("Preview: Report.pdf");
      expect(within(el).getByText("Report.pdf")).toBeInTheDocument();
      expect(within(el).queryByText("Preview")).not.toBeInTheDocument();
    });

    it.each([
      ["an activity block", (b: ReturnType<typeof block>) => lane({ blocks: [b] })],
      ["a summary block", (b: ReturnType<typeof block>) => summaryLane([b])],
    ])("shows the active time of %s, not the length of its span", async (_name, laneOf) => {
      const sparse = block({ id: "a6", app: "Preview", label: "Report.pdf", start: at("09:00"), end: at("09:30"), activeMs: 20 * MINUTE_MS });
      renderDay(dayView({ lanes: [laneOf(sparse)] }));
      const el = await blockEl("Preview: Report.pdf");
      expect(within(el).getByText("20m")).toBeInTheDocument();
    });

    it("shows hours in the duration of a long block", async () => {
      const long = block({ id: "a6", start: at("09:00"), end: at("11:05"), activeMs: 125 * MINUTE_MS });
      renderDay(dayView({ lanes: [lane({ blocks: [long] })] }));
      expect(within(await blockEl("Safari: Start Page")).getByText("2h 05m")).toBeInTheDocument();
    });

    it("shows only the app when the label is the app", async () => {
      const plain = block({ id: "a7", app: "Safari", title: "Docs", label: "Safari" });
      renderDay(dayView({ lanes: [lane({ blocks: [plain] })] }));
      const el = await blockEl("Safari");
      expect(within(el).getAllByText("Safari")).toHaveLength(1);
      expect(within(el).queryByText("Docs")).not.toBeInTheDocument();
    });

    it("marks the summary lane by its kind", async () => {
      renderDay(dayView({ lanes: [summaryLane([docs]), lane({ blocks: [todo] })] }));
      const el = await blockEl("Safari: Docs");
      expect(el.closest("[data-lane-id]")).toHaveClass("lane--summary");
      expect(screen.getByLabelText("Notes: Todo").closest("[data-lane-id]")).toHaveClass("lane--activity");
    });
  });

  describe("compact timeline", () => {
    it("draws the day at 60% of the zoom", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], compact: true }));
      expect(await blockEl("Safari: Docs")).toHaveStyle({ height: "45px" });
      expect(scrollBox()).toHaveClass("timeline--compact");
    });

    it("draws the full zoom otherwise", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      expect(scrollBox()).not.toHaveClass("timeline--compact");
    });
  });

  describe("view options", () => {
    const mail = { appKey: "com.apple.Mail", app: "Mail" };

    it("opens from the header and lists the hidden apps", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], hiddenApps: [mail] }));
      await blockEl("Safari: Docs");
      await userEvent.click(screen.getByRole("button", { name: "View options" }));
      const dialog = within(await screen.findByRole("dialog", { name: "View options" }));
      expect(dialog.getByRole("heading", { name: "Hidden apps (1)" })).toBeInTheDocument();
      expect(dialog.getByText("Mail")).toBeInTheDocument();
    });

    it("lists the apps of the day with a lane switch", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], apps: [dayApp()] }));
      await blockEl("Safari: Docs");
      await userEvent.click(screen.getByRole("button", { name: "View options" }));
      expect(await screen.findByRole("switch", { name: "Safari lane" })).not.toBeChecked();
    });

    it("shows a hidden app again with Unhide", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], hiddenApps: [mail] }));
      await blockEl("Safari: Docs");
      await userEvent.click(screen.getByRole("button", { name: "View options" }));
      await userEvent.click(await screen.findByRole("button", { name: "Unhide" }));
      expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Mail", { hidden: false });
    });

    it("opens with the day.settings command", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      act(() => {
        runCommand("day.settings");
      });
      expect(await screen.findByRole("dialog", { name: "View options" })).toBeInTheDocument();
    });
  });

  describe("empty and failed days", () => {
    it("says nothing was tracked on an empty past day", async () => {
      renderDay(dayView(), { tracker: trackerStatus({ text: "Tracking · Mail" }) });
      expect(await screen.findByText("Nothing tracked on this day")).toBeInTheDocument();
      expect(screen.queryByText("Tracking · Mail")).not.toBeInTheDocument();
    });

    it("adds the tracker status to an empty today", async () => {
      renderDay(dayView({ day: today() }), { tracker: trackerStatus({ text: "Tracking · Mail" }) });
      expect(await screen.findByText("Nothing tracked on this day")).toBeInTheDocument();
      expect(screen.getByText("Tracking · Mail")).toBeInTheDocument();
    });

    it("asks for the permission on an empty today while the tracker needs it", async () => {
      renderDay(dayView({ day: today() }), { tracker: trackerStatus({ state: { kind: "needsPermission" }, permission: "denied", text: "Needs permission" }) });
      expect(await screen.findByText("Tracking needs permission")).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Allow…" }));
      expect(await screen.findByRole("dialog", { name: "Allow TimeTracker to read window titles" })).toBeInTheDocument();
    });

    it("does not ask for the permission on a past day", async () => {
      renderDay(dayView(), { tracker: trackerStatus({ state: { kind: "needsPermission" }, permission: "denied", text: "Needs permission" }) });
      expect(await screen.findByText("Nothing tracked on this day")).toBeInTheDocument();
      expect(screen.queryByText("Tracking needs permission")).not.toBeInTheDocument();
    });

    it("keeps the hour axis and the lanes on an empty day, with the message in the lanes", async () => {
      renderDay(dayView({ lanes: [lane()] }));
      expect(await screen.findByText("Nothing tracked on this day")).toBeInTheDocument();
      expect(screen.getByText("09:00")).toBeInTheDocument();
      expect(screen.getByRole("columnheader")).toHaveTextContent("Activity");
    });

    it("opens the editor with the dragged span on an empty day", async () => {
      mocked.recordingSnap.mockResolvedValue({ start: at("09:00"), end: at("10:30") });
      renderDay(dayView({ lanes: [lane()] }));
      const message = await screen.findByText("Nothing tracked on this day");
      fireEvent.pointerDown(message, { button: 0, clientX: 300, clientY: px("09:02") });
      fireEvent.pointerMove(message, { clientX: 300, clientY: px("10:28") });
      fireEvent.pointerUp(message, { clientX: 300, clientY: px("10:28") });
      expectSpan(await editorForm("New recording"), "09:00", "10:30");
    });

    it("shows the error when the day cannot be loaded", async () => {
      mocked.dayView.mockRejectedValue({ kind: "db", message: "Database locked", code: null });
      resetStore({ day: DAY });
      render(<DayPage />);
      expect(await screen.findByText("Database locked")).toBeInTheDocument();
    });
  });

  describe("reloading", () => {
    it("loads the shown day again when its data changed, keeping the focused block", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      mocked.dayView.mockClear();
      act(() => useStore.getState().dataChanged({ kind: "activity", day: DAY }));
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenCalledWith(DAY));
      expect(screen.getByLabelText("Safari: Docs")).toHaveClass("focused");
    });

    it("ignores a change of another day", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      mocked.dayView.mockClear();
      act(() => useStore.getState().dataChanged({ kind: "activity", day: "2026-03-12" }));
      await new Promise((resolve) => setTimeout(resolve, 20));
      expect(mocked.dayView).not.toHaveBeenCalled();
    });

    it("follows today every 30 seconds", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      renderDay(dayView({ day: today() }));
      await screen.findByText("Nothing tracked on this day");
      mocked.dayView.mockClear();
      await act(() => vi.advanceTimersByTimeAsync(30_100));
      expect(mocked.dayView).toHaveBeenCalledWith(today());
    });

    it("does not poll a past day", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      renderDay(dayView());
      await screen.findByText("Nothing tracked on this day");
      mocked.dayView.mockClear();
      await act(() => vi.advanceTimersByTimeAsync(60_000));
      expect(mocked.dayView).not.toHaveBeenCalled();
    });

    it("reloads today after a tracker status change, at most every 10 seconds", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      renderDay(dayView({ day: today() }));
      await screen.findByText("Nothing tracked on this day");
      mocked.dayView.mockClear();
      act(() => useStore.setState({ tracker: trackerStatus({ text: "Tracking · Mail" }) }));
      await act(() => vi.advanceTimersByTimeAsync(5_000));
      expect(mocked.dayView).not.toHaveBeenCalled();
      await act(() => vi.advanceTimersByTimeAsync(5_200));
      expect(mocked.dayView).toHaveBeenCalledTimes(1);
    });

    describe("while the window is hidden", () => {
      const setVisibility = (state: "visible" | "hidden") => {
        Object.defineProperty(document, "visibilityState", { configurable: true, get: () => state });
        act(() => {
          document.dispatchEvent(new Event("visibilitychange"));
        });
      };

      afterEach(() => {
        delete (document as { visibilityState?: string }).visibilityState;
      });

      async function renderTodayHidden() {
        vi.useFakeTimers({ shouldAdvanceTime: true });
        renderDay(dayView({ day: today() }));
        await screen.findByText("Nothing tracked on this day");
        setVisibility("hidden");
        mocked.dayView.mockClear();
      }

      it("does not reload today, however long it stays hidden", async () => {
        await renderTodayHidden();
        await act(() => vi.advanceTimersByTimeAsync(95_000));
        expect(mocked.dayView).not.toHaveBeenCalled();
      });

      it("reloads once at once when shown again, then every 30 seconds", async () => {
        await renderTodayHidden();
        await act(() => vi.advanceTimersByTimeAsync(45_000));
        setVisibility("visible");
        await act(() => vi.advanceTimersByTimeAsync(0));
        expect(mocked.dayView).toHaveBeenCalledTimes(1);
        mocked.dayView.mockClear();
        await act(() => vi.advanceTimersByTimeAsync(15_000));
        expect(mocked.dayView).not.toHaveBeenCalled();
        await act(() => vi.advanceTimersByTimeAsync(15_200));
        expect(mocked.dayView).toHaveBeenCalledTimes(1);
      });

      it("catches up with one reload for a tracker change and adds none 10 seconds later", async () => {
        await renderTodayHidden();
        act(() => useStore.setState({ tracker: trackerStatus({ text: "Tracking · Mail" }) }));
        await act(() => vi.advanceTimersByTimeAsync(15_000));
        expect(mocked.dayView).not.toHaveBeenCalled();
        setVisibility("visible");
        await act(() => vi.advanceTimersByTimeAsync(0));
        expect(mocked.dayView).toHaveBeenCalledTimes(1);
        await act(() => vi.advanceTimersByTimeAsync(12_000));
        expect(mocked.dayView).toHaveBeenCalledTimes(1);
      });

      it("does not reload a past day when shown again", async () => {
        vi.useFakeTimers({ shouldAdvanceTime: true });
        renderDay(dayView());
        await screen.findByText("Nothing tracked on this day");
        mocked.dayView.mockClear();
        setVisibility("hidden");
        await act(() => vi.advanceTimersByTimeAsync(45_000));
        setVisibility("visible");
        await act(() => vi.advanceTimersByTimeAsync(60_000));
        expect(mocked.dayView).not.toHaveBeenCalled();
      });
    });
  });

  describe("day shortcuts", () => {
    it("loads the next day with N and the previous with P", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("n");
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenLastCalledWith("2026-03-11"));
      await userEvent.keyboard("pp");
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenLastCalledWith("2026-03-09"));
    });

    it("returns to today with O", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("o");
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenLastCalledWith(today()));
      expect(useStore.getState().day).toBe(today());
    });

    it("zooms the timeline with ⌘= and resets it with ⌘0", async () => {
      renderDay(busyDay());
      const el = await blockEl("Safari: Docs");
      expect(el).toHaveStyle({ height: "75px" });
      await userEvent.keyboard("{Meta>}={/Meta}");
      expect(el).toHaveStyle({ height: "90px" });
      await userEvent.keyboard("{Meta>}0{/Meta}");
      expect(el).toHaveStyle({ height: "75px" });
    });
  });

  describe("block focus", () => {
    const focusedLabels = () => screen.getAllByLabelText(/^\w+: /).filter((el) => el.classList.contains("focused")).map((el) => el.getAttribute("aria-label"));

    it("focuses the block that is clicked", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Notes: Todo"));
      expect(focusedLabels()).toEqual(["Notes: Todo"]);
    });

    it("moves to the next and previous block in time order with J and K", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("j");
      expect(focusedLabels()).toEqual(["Safari: Docs"]);
      await userEvent.keyboard("j");
      expect(focusedLabels()).toEqual(["Notes: Todo"]);
      await userEvent.keyboard("k");
      expect(focusedLabels()).toEqual(["Safari: Docs"]);
    });

    it("starts K from the last block", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("k");
      expect(focusedLabels()).toEqual(["Planner: Standup"]);
    });

    it("stays on the last block when J goes past it", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("jjjjj");
      expect(focusedLabels()).toEqual(["Planner: Standup"]);
    });

    it("moves between lanes with L and H", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("jj");
      await userEvent.keyboard("l");
      expect(focusedLabels()).toEqual(["Planner: Standup"]);
      await userEvent.keyboard("h");
      expect(focusedLabels()).toEqual(["Notes: Todo"]);
    });

    it("stays in the outermost lane when there is none further", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("jh");
      expect(focusedLabels()).toEqual(["Safari: Docs"]);
    });

    it("opens the card of the focused block with Space and closes it with another Space", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("j ");
      expect(screen.getByRole("dialog", { name: "Safari details" })).toBeInTheDocument();
      await userEvent.keyboard(" ");
      expect(screen.queryByRole("dialog", { name: "Safari details" })).not.toBeInTheDocument();
    });

    it("does nothing on Space without a focused block", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard(" ");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("does not open the card or the editor on Enter while a block has focus but nothing is selected", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("j{Enter}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("opens the block menu with Ctrl+Enter", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("j{Control>}{Enter}{/Control}");
      expect(await screen.findByRole("menuitemcheckbox", { name: "Own lane" })).toBeInTheDocument();
      expect(screen.queryByRole("dialog", { name: "Safari details" })).not.toBeInTheDocument();
    });

    it("clears the focus with Escape", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("j{Escape}");
      expect(focusedLabels()).toEqual([]);
    });
  });

  describe("hover card", () => {
    async function hover(label: string) {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
      const el = await blockEl(label);
      await user.hover(el);
      return { user, el };
    }

    const advance = (ms: number) => act(() => vi.advanceTimersByTimeAsync(ms));

    it("appears after 300 ms with app, title, url and time", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [block({ id: "a1", app: "Safari", title: "Docs", url: "https://example.com/docs", start: at("09:12"), end: at("09:40") })] })] }));
      await hover("Safari: Docs");
      await advance(200);
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      await advance(150);
      const card = within(screen.getByRole("dialog", { name: "Safari details" }));
      expect(card.getByText("Docs")).toBeInTheDocument();
      expect(card.getByRole("button", { name: "https://example.com/docs" })).toBeInTheDocument();
      expect(card.getByText("09:12–09:40 · 28m")).toBeInTheDocument();
    });

    it("opens the url of the card through the api", async () => {
      renderDay(busyDay());
      const { user } = await hover("Safari: Docs");
      await advance(350);
      await user.click(screen.getByRole("button", { name: "https://example.com/docs" }));
      expect(mocked.openUrl).toHaveBeenCalledWith("https://example.com/docs");
    });

    it("shows a url that is not http(s) as plain text, with the path", async () => {
      const file = block({ id: "a1", app: "Preview", title: "Report", url: "file:///tmp/report.pdf", path: "/tmp/report.pdf" });
      renderDay(dayView({ lanes: [lane({ blocks: [file] })] }));
      await hover("Preview: Report");
      await advance(350);
      expect(screen.getByText("file:///tmp/report.pdf")).toBeInTheDocument();
      expect(screen.getByText("/tmp/report.pdf")).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "file:///tmp/report.pdf" })).not.toBeInTheDocument();
    });

    it("names the source of an external block", async () => {
      renderDay(busyDay());
      await hover("Planner: Standup");
      await advance(350);
      expect(screen.getByText("10:00–11:00 · 1h 00m · planner")).toBeInTheDocument();
    });

    it("disappears after the pointer left", async () => {
      renderDay(busyDay());
      const { user } = await hover("Safari: Docs");
      await advance(350);
      await user.unhover(screen.getByLabelText("Safari: Docs"));
      await advance(300);
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });

  describe("block menu", () => {
    async function openMenu(label: string) {
      await userEvent.pointer({ keys: "[MouseRight]", target: await blockEl(label) });
      return screen.findByRole("menu");
    }

    it("offers the app settings, hide, copy and Make a rule for a tracker block with a url", async () => {
      renderDay(busyDay());
      const menu = within(await openMenu("Safari: Docs"));
      expect(menu.getAllByRole("menuitem").map((i) => i.textContent)).toEqual(["Hide Safari", "Copy title", "Copy URL", "Make a rule…"]);
      expect(menu.getAllByRole("menuitemcheckbox").map((i) => i.textContent)).toEqual(["Group by app", "Titles: Document name", "Titles: App name", "Own lane"]);
    });

    it("checks Group by app when the timeline groups by app", async () => {
      renderDay(busyDay());
      expect(within(await openMenu("Safari: Docs")).getByRole("menuitemcheckbox", { name: "Group by app" })).toHaveAttribute("aria-checked", "true");
    });

    it("omits Copy URL when the block has none", async () => {
      renderDay(busyDay());
      const menu = within(await openMenu("Notes: Todo"));
      expect(menu.queryByRole("menuitem", { name: "Copy URL" })).not.toBeInTheDocument();
    });

    it("offers only copying and Make a rule for an external block", async () => {
      renderDay(busyDay());
      const menu = within(await openMenu("Planner: Standup"));
      expect(menu.getAllByRole("menuitem").map((i) => i.textContent)).toEqual(["Copy title", "Make a rule…"]);
    });

    it("ends with Make a rule…, which opens the rule sheet for the block's app without a project", async () => {
      renderDay(busyDay(), withProject);
      await userEvent.click(within(await openMenu("Notes: Todo")).getByRole("menuitem", { name: "Make a rule…" }));
      const sheet = within(await screen.findByRole("dialog", { name: "New rule" }));
      expect(sheet.getByLabelText("Name")).toHaveValue("Notes");
      expect(sheet.getByRole("combobox", { name: "Project" })).toHaveValue("");
      expect(sheet.getByLabelText("Condition 1 field")).toHaveValue("app");
      expect(sheet.getByLabelText("Condition 1 value")).toHaveValue("Notes");
      expect(mocked.ruleSave).not.toHaveBeenCalled();
    });

    it("moves the app to its own lane", async () => {
      renderDay(busyDay());
      const ownLane = within(await openMenu("Safari: Docs")).getByRole("menuitemcheckbox", { name: "Own lane" });
      expect(ownLane).toHaveAttribute("aria-checked", "false");
      await userEvent.click(ownLane);
      expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Safari", { ownLane: true });
    });

    it("moves an app lane's block back to the activity lane", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [todo] }), appLane("Safari", "com.apple.Safari", [docs])] }));
      const ownLane = within(await openMenu("Safari: Docs")).getByRole("menuitemcheckbox", { name: "Own lane" });
      expect(ownLane).toHaveAttribute("aria-checked", "true");
      await userEvent.click(ownLane);
      expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Safari", { ownLane: false });
    });

    it("opens the same menu from the gear of a block", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.click(screen.getByRole("button", { name: "Actions for Safari" }));
      expect(await screen.findByRole("menuitemcheckbox", { name: "Group by app" })).toBeInTheDocument();
    });

    it("hides the app and offers Undo, which shows it again", async () => {
      renderDay(busyDay());
      await userEvent.click(within(await openMenu("Safari: Docs")).getByRole("menuitem", { name: "Hide Safari" }));
      expect(mocked.setAppSetting).toHaveBeenLastCalledWith("com.apple.Safari", { hidden: true });
      expect(await screen.findByText("Safari is hidden from the timeline")).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Undo" }));
      expect(mocked.setAppSetting).toHaveBeenLastCalledWith("com.apple.Safari", { hidden: false });
    });

    it("shows the error and no Undo when hiding fails", async () => {
      renderDay(busyDay());
      mocked.setAppSetting.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await userEvent.click(within(await openMenu("Safari: Docs")).getByRole("menuitem", { name: "Hide Safari" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("Disk full");
      expect(screen.queryByRole("button", { name: "Undo" })).not.toBeInTheDocument();
      expect(screen.queryByText("Safari is hidden from the timeline")).not.toBeInTheDocument();
    });

    it("copies the title", async () => {
      renderDay(busyDay());
      const writeText = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue(undefined);
      await userEvent.click(within(await openMenu("Safari: Docs")).getByRole("menuitem", { name: "Copy title" }));
      expect(writeText).toHaveBeenCalledWith("Docs");
      expect(await screen.findByText("Copied the title")).toBeInTheDocument();
    });

    it("copies the url", async () => {
      renderDay(busyDay());
      const writeText = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue(undefined);
      await userEvent.click(within(await openMenu("Safari: Docs")).getByRole("menuitem", { name: "Copy URL" }));
      expect(writeText).toHaveBeenCalledWith("https://example.com/docs");
    });
  });

  describe("selection", () => {
    const editorDialog = () => editorForm("New recording");

    it("outlines the span of a clicked block until the editor has its draft", async () => {
      mocked.recordingDraftSpans.mockReturnValueOnce(new Promise(() => {}));
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      expect(await screen.findByLabelText("Selection 09:00–09:30")).toBeEmptyDOMElement();
    });

    it("shows a draft band for each span of the open editor, without text", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      await editorDialog();
      expect(screen.queryByLabelText(/^Selection/)).not.toBeInTheDocument();
      expect(lanesEl().querySelectorAll(".selection")).toHaveLength(1);
      await userEvent.click(screen.getByLabelText("Notes: Todo"));
      await waitFor(() => expect(lanesEl().querySelectorAll(".selection")).toHaveLength(2));
      for (const band of lanesEl().querySelectorAll(".selection")) {
        expect(band).toHaveAttribute("aria-hidden", "true");
        expect(band).toBeEmptyDOMElement();
      }
    });

    it("shows no time text in the lanes", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      await editorDialog();
      await userEvent.click(screen.getByLabelText("Notes: Todo"));
      await waitFor(() => expectSpan(screen.getByRole("form", { name: "New recording" }), "10:00", "10:30", 2));
      expect(within(lanesEl()).queryByText(/\d\d:\d\d/)).not.toBeInTheDocument();
    });

    it("marks the clicked blocks as selected and unmarks them when clicked again", async () => {
      renderDay(busyDay());
      const docsEl = await blockEl("Safari: Docs");
      const todoEl = screen.getByLabelText("Notes: Todo");
      await userEvent.click(docsEl);
      await editorDialog();
      await userEvent.click(todoEl);
      await waitFor(() => expect(todoEl).toHaveClass("selected"));
      expect(docsEl).toHaveClass("selected");
      await userEvent.click(docsEl);
      await waitFor(() => expect(docsEl).not.toHaveClass("selected"));
      expect(todoEl).toHaveClass("selected");
    });

    it("opens the editor in the timesheet with the span of the clicked block", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      const form = await editorDialog();
      expect(sheetOf()).toContainElement(form);
      expectSpan(form, "09:00", "09:30");
      expect(within(form).getByLabelText("Note")).toHaveFocus();
    });

    it("opens the editor on the span Rust snapped the block's span to", async () => {
      mocked.recordingSnap.mockResolvedValue({ start: at("08:55"), end: at("09:35") });
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      expectSpan(await editorDialog(), "08:55", "09:35");
      expect(mocked.recordingSnap).toHaveBeenCalledWith(at("09:00"), at("09:30"), null);
    });

    it("adds the span of a second clicked block as another row and keeps the note", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      const form = await editorDialog();
      await userEvent.type(within(form).getByLabelText("Note"), "keep me");
      await userEvent.click(screen.getByLabelText("Notes: Todo"));
      await waitFor(() => expectSpan(form, "10:00", "10:30", 2));
      expectSpan(form, "09:00", "09:30");
      expect(within(form).getByLabelText("Note")).toHaveValue("keep me");
      expect(screen.getAllByRole("form")).toHaveLength(1);
    });

    it("shows two draft blocks for two clicked blocks and leaves the time between them unassigned", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      await editorDialog();
      await userEvent.click(screen.getByLabelText("Notes: Todo"));
      await waitFor(() => expect(lanesEl().querySelectorAll(".selection")).toHaveLength(2));
      expect(screen.queryByLabelText(/^Selection/)).not.toBeInTheDocument();
      expect(await screen.findByLabelText("Draft 09:00–09:30")).toBeInTheDocument();
      expect(screen.getByLabelText("Draft 10:00–10:30")).toBeInTheDocument();
      expect(screen.queryByLabelText("Draft 09:30–10:00")).not.toBeInTheDocument();
    });

    it("removes the span of a block clicked a second time", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      const form = await editorDialog();
      await userEvent.click(screen.getByLabelText("Notes: Todo"));
      await waitFor(() => expectSpan(form, "10:00", "10:30", 2));
      await userEvent.click(screen.getByLabelText("Safari: Docs"));
      await waitFor(() => expect(within(form).queryByLabelText("Time 2 from")).not.toBeInTheDocument());
      expectSpan(form, "10:00", "10:30");
    });

    it("closes the editor without saving when the only block is clicked again", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      await editorDialog();
      await userEvent.click(screen.getByLabelText("Safari: Docs"));
      await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });

    it.each([
      ["a ⇧-click", { shiftKey: true }],
      ["a ⌘-click", { metaKey: true }],
      ["an ⌥-click", { altKey: true }],
    ])("treats %s on a block like a plain click", async (_name, modifier) => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      const form = await editorDialog();
      const todoEl = screen.getByLabelText("Notes: Todo");
      fireEvent.pointerDown(todoEl, { button: 0, ...modifier });
      fireEvent.pointerUp(todoEl, modifier);
      await waitFor(() => expectSpan(form, "10:00", "10:30", 2));
      expectSpan(form, "09:00", "09:30");
    });

    it("opens the editor on a modified first click without waiting for a key release", async () => {
      renderDay(busyDay());
      const el = await blockEl("Safari: Docs");
      fireEvent.pointerDown(el, { button: 0, metaKey: true });
      fireEvent.pointerUp(el, { metaKey: true });
      expectSpan(await editorDialog(), "09:00", "09:30");
    });

    it("closes the editor and clears the selection with Escape", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      await editorDialog();
      await waitFor(() => expect(lanesEl().querySelectorAll(".selection")).toHaveLength(1));
      await userEvent.keyboard("{Escape}");
      expect(lanesEl().querySelectorAll(".selection")).toHaveLength(0);
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
      expect(useStore.getState().draft).toBeNull();
    });

    it("saves the recording of a clicked block with the project, note, tag and span", async () => {
      renderDay(busyDay(), withProject);
      await userEvent.click(await blockEl("Safari: Docs"));
      const form = within(await editorDialog());
      await userEvent.type(form.getByRole("combobox", { name: "Project" }), "Web{Enter}");
      await userEvent.type(form.getByLabelText("Note"), "read the docs");
      await userEvent.type(form.getByRole("combobox", { name: "Tag" }), "res{Enter}");
      await userEvent.click(form.getByRole("button", { name: "Save" }));
      await waitFor(() =>
        expect(mocked.recordingCreate).toHaveBeenCalledWith({ projectId: "p-site", spans: [{ start: at("09:00"), end: at("09:30") }], note: "read the docs", tagId: "tag-research" }, "cancel"),
      );
      await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
      expect(lanesEl().querySelectorAll(".selection")).toHaveLength(0);
    });

    it("opens a fresh editor on the next click after Cancel", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      await editorDialog();
      await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
      await userEvent.click(screen.getByLabelText("Notes: Todo"));
      expectSpan(await editorDialog(), "10:00", "10:30");
    });

    it("clears a pending selection when empty space is clicked", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("jv");
      await screen.findByLabelText("Selection 09:00–09:30");
      await userEvent.click(lanesEl());
      expect(screen.queryByLabelText(/^Selection/)).not.toBeInTheDocument();
    });

    describe("dragging", () => {
      /** Presses `target` at 09:02 and drags to 10:28 (Rust snaps it to 09:00–10:30); the returned function releases. */
      async function dragOver(target: () => HTMLElement) {
        mocked.recordingSnap.mockResolvedValue({ start: at("09:00"), end: at("10:30") });
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        fireEvent.pointerDown(target(), { button: 0, clientX: 300, clientY: px("09:02") });
        fireEvent.pointerMove(target(), { clientX: 300, clientY: px("10:28") });
        const band = await screen.findByLabelText("Selection 09:00–10:30");
        return { band, release: () => fireEvent.pointerUp(target(), { clientX: 300, clientY: px("10:28") }) };
      }

      it("shows the dragged span with the times Rust snaps it to, as a band without text", async () => {
        const { band } = await dragOver(lanesEl);
        expect(band).toBeEmptyDOMElement();
        expect(mocked.recordingSnap).toHaveBeenCalledWith(at("09:02"), at("10:28"), "end");
      });

      it("opens the editor with the snapped span when released over the lanes", async () => {
        const { release } = await dragOver(lanesEl);
        release();
        expectSpan(await editorDialog(), "09:00", "10:30");
      });

      it("opens the editor with the snapped span after a drag in the time column", async () => {
        const { release } = await dragOver(() => screen.getByLabelText("Recordings"));
        release();
        expectSpan(await editorDialog(), "09:00", "10:30");
      });

      it("opens no editor for a press in the time column that was not moved", async () => {
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        const column = screen.getByLabelText("Recordings");
        fireEvent.pointerDown(column, { button: 0, clientY: px("09:02") });
        fireEvent.pointerUp(column, { clientY: px("09:02") });
        expect(screen.queryByRole("form")).not.toBeInTheDocument();
      });

      it("adds a row to the open editor with a drag", async () => {
        renderDay(busyDay());
        await userEvent.click(await blockEl("Safari: Docs"));
        const form = await editorDialog();
        fireEvent.pointerDown(lanesEl(), { button: 0, clientX: 300, clientY: px("13:00") });
        fireEvent.pointerMove(lanesEl(), { clientX: 300, clientY: px("13:30") });
        fireEvent.pointerUp(lanesEl(), { clientX: 300, clientY: px("13:30") });
        await waitFor(() => expectSpan(form, "13:00", "13:30", 2));
        expectSpan(form, "09:00", "09:30");
        expect(screen.getAllByRole("form")).toHaveLength(1);
      });
    });
  });

  describe("recordings", () => {
    const recordingEl = () => screen.findByLabelText("Website: standup");
    const grip = (el: HTMLElement, which: "start" | "end") => within(el).getByLabelText(`Drag to change the ${which}`);

    it("shows a recording in the time column by its start and end only", async () => {
      renderDay(dayView({ recordings: [{ ...meeting, tag: "support" }] }));
      const el = within(await recordingEl());
      expect(screen.getByLabelText("Recordings")).toContainElement(screen.getByLabelText("Website: standup"));
      expect(el.getByText("11:00")).toBeInTheDocument();
      expect(el.getByText("11:45")).toBeInTheDocument();
      for (const text of ["Website", "standup", "support", "45m"]) expect(el.queryByText(text)).not.toBeInTheDocument();
    });

    it("shows each span of a recording with several by its own times", async () => {
      const morning = recordingBlock({ id: "r1", spanId: "r1-s1", spanCount: 2, start: at("09:00"), end: at("09:45") });
      const noon = recordingBlock({ id: "r1", spanId: "r1-s2", spanCount: 2, start: at("11:00"), end: at("11:45") });
      renderDay(dayView({ recordings: [morning, noon] }));
      const column = within(await screen.findByLabelText("Recordings"));
      for (const time of ["09:00", "09:45", "11:00", "11:45"]) expect(column.getByText(time)).toBeInTheDocument();
    });

    it("shows only the start of a span too short for both times", async () => {
      renderDay(dayView({ recordings: [recordingBlock({ id: "r3", note: "quick", start: at("10:00"), end: at("10:10") })] }));
      const el = within(await screen.findByLabelText("Website: quick"));
      expect(el.getByText("10:00")).toBeInTheDocument();
      expect(el.queryByText("10:10")).not.toBeInTheDocument();
    });

    it("shows no times in a span too short for text", async () => {
      renderDay(dayView({ recordings: [recordingBlock({ id: "r3", note: "quick", start: at("10:00"), end: at("10:05") })] }));
      const el = within(await screen.findByLabelText("Website: quick"));
      expect(el.queryByText("10:00")).not.toBeInTheDocument();
    });

    it("shows a day with only recordings as a timeline", async () => {
      renderDay(dayView({ recordings: [meeting] }));
      expect(await recordingEl()).toBeInTheDocument();
      expect(screen.queryByText("Nothing tracked on this day")).not.toBeInTheDocument();
    });

    it("dims the activity blocks that recordings cover", async () => {
      const covered = block({ id: "a4", app: "Mail", title: "Inbox", covered: true });
      renderDay(dayView({ lanes: [lane({ blocks: [docs, covered] })] }));
      expect(await blockEl("Mail: Inbox")).toHaveClass("dimmed");
      expect(screen.getByLabelText("Safari: Docs")).not.toHaveClass("dimmed");
    });

    it("marks a running recording and gives it no grips", async () => {
      renderDay(dayView({ recordings: [{ ...meeting, running: true }] }));
      const el = await recordingEl();
      expect(el).toHaveClass("running");
      expect(within(el).queryByLabelText(/^Drag to change/)).not.toBeInTheDocument();
    });

    it("does not mark a finished recording as running", async () => {
      renderDay(recordedDay());
      expect(await recordingEl()).not.toHaveClass("running");
    });

    it("opens the editor on the recording with a single click and draws its draft instead of the block", async () => {
      mocked.recordingsList.mockResolvedValue([meetingRow]);
      renderDay(recordedDay(), withProject);
      await userEvent.click(await recordingEl());
      const form = await editorForm("Edit recording");
      expectSpan(form, "11:00", "11:45");
      expect(screen.getByLabelText("Draft 11:00–11:45")).toBeInTheDocument();
      expect(screen.queryByLabelText("Website: standup")).not.toBeInTheDocument();
    });

    it("opens the editor with every span of a recording that has several as a row", async () => {
      const morning = { id: "r1-s1", start: at("09:00"), end: at("09:30") };
      const noon = { id: "r1-s2", start: at("11:00"), end: at("11:45") };
      mocked.recordingsList.mockResolvedValue([recording({ ...meetingRow, spans: [morning, noon] })]);
      renderDay(dayView({ recordings: [{ ...meeting, spanId: noon.id, spanCount: 2 }] }), withProject);
      await userEvent.click(await recordingEl());
      const form = await editorForm("Edit recording");
      expectSpan(form, "09:00", "09:30");
      expectSpan(form, "11:00", "11:45", 2);
      expect(screen.getByLabelText("Draft 09:00–09:30")).toBeInTheDocument();
      expect(screen.getByLabelText("Draft 11:00–11:45")).toBeInTheDocument();
    });

    describe("dragging", () => {
      it.each([
        ["its body moves it", null, 25, "11:10", "11:55"],
        ["its top edge changes the start", "start", -25, "10:50", "11:45"],
        ["its bottom edge changes the end", "end", 25, "11:00", "11:55"],
      ] as const)("%s, sending the raw span and the dragged edge for Rust to snap", async (_name, edge, dy, start, end) => {
        renderDay(recordedDay(), withProject);
        const el = await recordingEl();
        const handle = edge ? grip(el, edge) : el;
        fireEvent.pointerDown(handle, { button: 0, clientY: 1000 });
        fireEvent.pointerMove(handle, { clientY: 1000 + dy });
        fireEvent.pointerUp(handle, { clientY: 1000 + dy });
        await vi.waitFor(() => expect(mocked.recordingUpdateSpan).toHaveBeenCalledWith("r1", "r1-s1", { start: at(start), end: at(end) }, edge, "cancel"));
        expect(await screen.findByText("Recording changed")).toBeInTheDocument();
      });

      it("changes the span of the dragged block when the recording has several", async () => {
        const second = { ...meeting, spanId: "r1-s2", spanCount: 2 };
        renderDay(dayView({ recordings: [second] }), withProject);
        const el = await recordingEl();
        fireEvent.pointerDown(el, { button: 0, clientY: 1000 });
        fireEvent.pointerMove(el, { clientY: 1025 });
        fireEvent.pointerUp(el, { clientY: 1025 });
        await vi.waitFor(() => expect(mocked.recordingUpdateSpan).toHaveBeenCalledWith("r1", "r1-s2", { start: at("11:10"), end: at("11:55") }, null, "cancel"));
      });

      it("changes nothing when the pointer is released where it was pressed", async () => {
        renderDay(recordedDay(), withProject);
        const handle = grip(await recordingEl(), "end");
        fireEvent.pointerDown(handle, { button: 0, clientY: 1000 });
        fireEvent.pointerUp(handle, { clientY: 1000 });
        expect(mocked.recordingUpdateSpan).not.toHaveBeenCalled();
      });

      it("does not move a recording that continues on another day", async () => {
        renderDay(dayView({ recordings: [{ ...meeting, clippedEnd: true }] }), withProject);
        const el = await recordingEl();
        fireEvent.pointerDown(el, { button: 0, clientY: 1000 });
        fireEvent.pointerMove(el, { clientY: 1025 });
        fireEvent.pointerUp(el, { clientY: 1025 });
        expect(within(el).queryByLabelText(/^Drag to change/)).not.toBeInTheDocument();
        expect(mocked.recordingUpdateSpan).not.toHaveBeenCalled();
      });

      it("shows an error toast when Rust refuses the new times", async () => {
        mocked.recordingUpdateSpan.mockRejectedValueOnce({ kind: "invalid_input", message: "`end`: too long", code: null, field: { path: "end", message: "Too long" } });
        renderDay(recordedDay(), withProject);
        const handle = grip(await recordingEl(), "end");
        fireEvent.pointerDown(handle, { button: 0, clientY: 1000 });
        fireEvent.pointerMove(handle, { clientY: 1025 });
        fireEvent.pointerUp(handle, { clientY: 1025 });
        expect(await screen.findByRole("alert")).toHaveTextContent("Could not change the recording");
      });
    });

    describe("keys", () => {
      it.each([
        ["Alt+↑ moves it one snap step earlier", "{Alt>}{ArrowUp}{/Alt}", -1, -1],
        ["Alt+↓ moves it one snap step later", "{Alt>}{ArrowDown}{/Alt}", 1, 1],
        ["Alt+Shift+↓ extends its end by one step", "{Alt>}{Shift>}{ArrowDown}{/Shift}{/Alt}", 0, 1],
        ["Alt+Shift+↑ shortens its end by one step", "{Alt>}{Shift>}{ArrowUp}{/Shift}{/Alt}", 0, -1],
      ])("%s", async (_name, keys, startSteps, endSteps) => {
        renderDay(recordedDay(), withProject);
        await recordingEl();
        await userEvent.keyboard("]");
        await userEvent.keyboard(keys);
        await vi.waitFor(() => expect(mocked.recordingNudge).toHaveBeenCalledWith("r1", "r1-s1", startSteps, endSteps, "cancel"));
      });

      it("does not nudge a recording that continues on another day", async () => {
        renderDay(withTimesheet(dayView({ recordings: [{ ...meeting, clippedStart: true }] })), withProject);
        await recordingEl();
        await userEvent.keyboard("]");
        await userEvent.keyboard("{Alt>}{ArrowDown}{/Alt}");
        expect(mocked.recordingNudge).not.toHaveBeenCalled();
      });

      it("deletes the focused recording with Backspace and offers Undo", async () => {
        renderDay(recordedDay(), withProject);
        await recordingEl();
        await userEvent.keyboard("]");
        await userEvent.keyboard("{Backspace}");
        await vi.waitFor(() => expect(mocked.recordingDelete).toHaveBeenCalledWith("r1"));
        expect(await screen.findByText("Recording deleted")).toBeInTheDocument();
        mocked.undo.mockResolvedValueOnce("Delete recording");
        await userEvent.click(screen.getByRole("button", { name: "Undo" }));
        expect(mocked.undo).toHaveBeenCalled();
        expect(await screen.findByText("Undid “Delete recording”")).toBeInTheDocument();
      });

      it("does not delete on Backspace without a focused recording", async () => {
        renderDay(recordedDay(), withProject);
        await recordingEl();
        await userEvent.keyboard("{Backspace}");
        expect(mocked.recordingDelete).not.toHaveBeenCalled();
      });

      it("undoes with ⌘Z and redoes with ⌘⇧Z", async () => {
        mocked.undo.mockResolvedValueOnce("Move recording");
        mocked.redo.mockResolvedValueOnce("Move recording");
        renderDay(recordedDay(), withProject);
        await recordingEl();
        await userEvent.keyboard("{Meta>}z{/Meta}");
        expect(await screen.findByText("Undid “Move recording”")).toBeInTheDocument();
        await userEvent.keyboard("{Meta>}{Shift>}z{/Shift}{/Meta}");
        expect(await screen.findByText("Redid “Move recording”")).toBeInTheDocument();
      });

      it("clears the focused recording with Escape", async () => {
        renderDay(recordedDay(), withProject);
        await recordingEl();
        await userEvent.keyboard("]");
        await userEvent.keyboard("{Escape}{Backspace}");
        expect(mocked.recordingDelete).not.toHaveBeenCalled();
      });
    });
  });

  describe("timesheet", () => {
    const planning = timesheetCard({ id: "r0", projectId: "p-site", projectName: "Website", note: "planning", start: at("09:00"), end: at("09:30") });
    const standupCard = timesheetCard({ id: "r1", projectId: "p-site", projectName: "Website", clientName: "Acme", note: "standup", tag: "support", start: at("11:00"), end: at("11:45") });
    const bare = timesheetCard({ id: "r2", projectName: "Docs", clientName: "Acme", start: at("13:00"), end: at("14:05") });
    const sheet = () => within(screen.getByRole("complementary", { name: "Timesheet" }));
    const cards = () => sheet().getAllByRole("button", { name: /, (\d+h )?\d+m$/ });
    const cardOf = (name: string) => sheet().getByRole("button", { name });

    const sheetDay = (timesheet = [planning, standupCard]) => dayView({ lanes: [lane({ blocks: [docs] })], recordings: [meeting], timesheet });

    it("lists a card per recording in the order of their first spans", async () => {
      renderDay(sheetDay([planning, standupCard, bare]));
      await screen.findByRole("complementary", { name: "Timesheet" });
      expect(cards().map((c) => c.getAttribute("aria-label"))).toEqual(["planning, Website, 30m", "standup, Website | Acme, 45m", "Docs, Docs | Acme, 1h 05m"]);
    });

    it("shows a card with the note as title, project and client, tag and duration", async () => {
      renderDay(sheetDay());
      await screen.findByRole("complementary", { name: "Timesheet" });
      const card = within(cardOf("standup, Website | Acme, 45m"));
      expect(card.getByText("standup")).toBeInTheDocument();
      expect(card.getByText("Website | Acme")).toBeInTheDocument();
      expect(card.getByText("support")).toBeInTheDocument();
      expect(card.getByText("45m")).toBeInTheDocument();
    });

    it("titles a card without a note by its project and shows the client below", async () => {
      renderDay(sheetDay([bare]));
      await screen.findByRole("complementary", { name: "Timesheet" });
      const card = within(cardOf("Docs, Docs | Acme, 1h 05m"));
      expect(card.getByText("Docs")).toBeInTheDocument();
      expect(card.getByText("Acme")).toBeInTheDocument();
      expect(card.getByText("1h 05m")).toBeInTheDocument();
    });

    it("counts the times of a recording with several", async () => {
      const split = timesheetCard({ id: "r3", note: "pairing", spanIds: ["r3-s1", "r3-s2"] });
      renderDay(sheetDay([planning, split]));
      await screen.findByRole("complementary", { name: "Timesheet" });
      expect(within(cardOf("pairing, Website, 45m")).getByText("2 times")).toBeInTheDocument();
      expect(within(cardOf("planning, Website, 30m")).queryByText(/times/)).not.toBeInTheDocument();
    });

    it("shows the recorded time of the day as the total", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], timesheet: [planning, standupCard], totals: { recordedMs: 75 * MINUTE_MS, trackedMs: 0, awayMs: 0 } }));
      await screen.findByRole("complementary", { name: "Timesheet" });
      expect(sheet().getByLabelText("Recorded")).toHaveTextContent("1h 15m");
    });

    it("says nothing is recorded yet when the day has no recordings", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      expect(sheet().getByText("Nothing recorded yet. Drag over the timeline or click an activity.")).toBeInTheDocument();
      expect(sheet().getByLabelText("Recorded")).toHaveTextContent("0m");
    });

    it("stays next to the message of a day without activity", async () => {
      renderDay(dayView());
      expect(await screen.findByText("Nothing tracked on this day")).toBeInTheDocument();
      expect(sheet().getByText(/Nothing recorded yet/)).toBeInTheDocument();
    });

    describe("a card", () => {
      it("is focused by a click, which also focuses its span in the time column", async () => {
        renderDay(sheetDay());
        const card = await screen.findByRole("button", { name: "standup, Website | Acme, 45m" });
        expect(card).toHaveAttribute("aria-pressed", "false");
        await userEvent.click(card);
        expect(card).toHaveAttribute("aria-pressed", "true");
        expect(screen.getByLabelText("Website: standup")).toHaveClass("focused");
        expect(cardOf("planning, Website, 30m")).toHaveAttribute("aria-pressed", "false");
      });

      it("gives way to the editor of its recording when clicked", async () => {
        mocked.recordingsList.mockResolvedValue([meetingRow]);
        renderDay(sheetDay(), withProject);
        await userEvent.click(await screen.findByRole("button", { name: "standup, Website | Acme, 45m" }));
        const form = await editorForm("Edit recording");
        expectSpan(form, "11:00", "11:45");
        expect(sheet().queryByRole("button", { name: "standup, Website | Acme, 45m" })).not.toBeInTheDocument();
        expect(cardOf("planning, Website, 30m")).toBeInTheDocument();
      });

      it("opens the editor with every span of a recording with several as a row", async () => {
        const morning = { id: "r1-s1", start: at("09:00"), end: at("09:30") };
        const noon = { id: "r1-s2", start: at("11:00"), end: at("11:45") };
        mocked.recordingsList.mockResolvedValue([recording({ ...meetingRow, spans: [morning, noon] })]);
        const split = timesheetCard({ id: "r1", projectId: "p-site", note: "standup", start: at("09:00"), end: at("11:45"), durationMs: 75 * MINUTE_MS, spanIds: [morning.id, noon.id] });
        renderDay(sheetDay([split]), withProject);
        await userEvent.click(await screen.findByRole("button", { name: "standup, Website, 1h 15m" }));
        const form = await editorForm("Edit recording");
        expectSpan(form, "09:00", "09:30");
        expectSpan(form, "11:00", "11:45", 2);
      });

      it("opens the editor with Enter on the focused card", async () => {
        mocked.recordingsList.mockResolvedValue([meetingRow]);
        renderDay(sheetDay(), withProject);
        (await screen.findByRole("button", { name: "standup, Website | Acme, 45m" })).focus();
        await userEvent.keyboard("{Enter}");
        expectSpan(await editorForm("Edit recording"), "11:00", "11:45");
      });
    });

    describe("the open editor", () => {
      it("sits between the cards by the start of a new recording", async () => {
        renderDay(sheetDay(), withProject);
        await userEvent.click(await blockEl("Safari: Docs"));
        const form = await editorForm("New recording");
        const after = (a: Element, b: Element) => Boolean(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);
        expect(after(cardOf("planning, Website, 30m"), form)).toBe(true);
        expect(after(form, cardOf("standup, Website | Acme, 45m"))).toBe(true);
      });

      it("replaces the empty message of a day without recordings", async () => {
        renderDay(busyDay(), withProject);
        await userEvent.click(await blockEl("Safari: Docs"));
        await editorForm("New recording");
        expect(sheet().queryByText(/Nothing recorded yet/)).not.toBeInTheDocument();
      });
    });

    describe("hovering", () => {
      it("marks the spans of the hovered card in the time column and clears them when it is left", async () => {
        renderDay(sheetDay());
        const card = await screen.findByRole("button", { name: "standup, Website | Acme, 45m" });
        fireEvent.pointerEnter(card);
        expect(card).toHaveClass("hovered");
        expect(screen.getByLabelText("Website: standup")).toHaveClass("hovered");
        fireEvent.pointerLeave(card);
        expect(card).not.toHaveClass("hovered");
        expect(screen.getByLabelText("Website: standup")).not.toHaveClass("hovered");
      });

      it("marks the card of a hovered span in the time column", async () => {
        renderDay(sheetDay());
        const span = await blockEl("Website: standup");
        fireEvent.pointerEnter(span);
        expect(cardOf("standup, Website | Acme, 45m")).toHaveClass("hovered");
        expect(cardOf("planning, Website, 30m")).not.toHaveClass("hovered");
      });
    });

    describe("the + button", () => {
      it("opens the editor for a new recording on the default span", async () => {
        mocked.recordingDefaultSpan.mockResolvedValue({ start: at("14:00"), end: at("14:30") });
        renderDay(busyDay(), withProject);
        mocked.dayView.mockImplementation(async (day) => ({ ...busyDay(), day }));
        await userEvent.click(await screen.findByRole("button", { name: "New recording" }));
        expectSpan(await editorForm("New recording"), "14:00", "14:30");
      });

      it("opens the editor on the selection when there is one", async () => {
        renderDay(busyDay(), withProject);
        await blockEl("Safari: Docs");
        await userEvent.keyboard("jv");
        await userEvent.click(screen.getByRole("button", { name: "New recording" }));
        expectSpan(await editorForm("New recording"), "09:00", "09:30");
      });
    });
  });

  describe("zoom control", () => {
    const docsEl = () => blockEl("Safari: Docs");

    it("zooms in one step with + and out with −", async () => {
      renderDay(busyDay());
      expect(await docsEl()).toHaveStyle({ height: "75px" });
      await userEvent.click(screen.getByRole("button", { name: "Zoom in" }));
      expect(await docsEl()).toHaveStyle({ height: "90px" });
      await userEvent.click(screen.getByRole("button", { name: "Zoom out" }));
      await userEvent.click(screen.getByRole("button", { name: "Zoom out" }));
      expect(await docsEl()).toHaveStyle({ height: "60px" });
    });

    it("sets the scale with the slider", async () => {
      renderDay(busyDay());
      await docsEl();
      fireEvent.change(screen.getByRole("slider", { name: "Zoom level" }), { target: { value: "4" } });
      expect(await docsEl()).toHaveStyle({ height: "120px" });
      expect(screen.getByRole("slider", { name: "Zoom level" })).toHaveValue("4");
    });

    it("stops zooming in at the largest scale", async () => {
      renderDay(busyDay(), { uiState: { ...useStore.getState().uiState, timelineZoom: 8 } });
      await docsEl();
      expect(screen.getByRole("button", { name: "Zoom in" })).toBeDisabled();
      expect(screen.getByRole("button", { name: "Zoom out" })).toBeEnabled();
    });

    it("stops zooming out at the smallest scale", async () => {
      renderDay(busyDay(), { uiState: { ...useStore.getState().uiState, timelineZoom: 1 } });
      expect(await docsEl()).toHaveStyle({ height: "30px" });
      expect(screen.getByRole("button", { name: "Zoom out" })).toBeDisabled();
      expect(screen.getByRole("button", { name: "Zoom in" })).toBeEnabled();
    });
  });

  describe("away time", () => {
    it("a day with only away time shows the empty state", async () => {
      const totals = { recordedMs: 0, trackedMs: 0, awayMs: 30 * MINUTE_MS };
      renderDay(dayView({ totals }));
      expect(await screen.findByText("Nothing tracked on this day")).toBeInTheDocument();
      expect(screen.queryByText(/^Away ·/)).not.toBeInTheDocument();
    });
  });

  describe("suggestions", () => {
    const review = timesheetCard({ id: "c1", projectId: "p-site", note: "review", start: at("10:00"), end: at("10:30") });
    const standupIdea = suggestion({ ruleId: "rule-slack", ruleName: "Slack", projectId: "p-site", projectName: "Website", note: "standup", start: at("09:00"), end: at("09:45"), nextCard: "c1" });
    const mailIdea = suggestion({ ruleId: "rule-mail", ruleName: "Mail", projectId: "p-site", projectName: "Website", start: at("14:00"), end: at("14:30"), nextCard: null });
    const withSuggestions = () => dayView({ lanes: [lane({ blocks: [docs] })], timesheet: [review], suggestions: [standupIdea, mailIdea] });
    const standupCard = () => screen.findByRole("group", { name: /^Suggested: standup/ });
    const mailCard = () => screen.findByRole("group", { name: /^Suggested: Website, Website/ });
    const focusCard = async (card: Promise<HTMLElement>) => {
      const el = await card;
      act(() => el.focus());
    };

    describe("ghost cards", () => {
      it("draws one per suggestion in the timesheet with the rule that suggested it", async () => {
        renderDay(withSuggestions(), withProject);
        const card = within(await standupCard());
        expect(sheetOf()).toContainElement(await standupCard());
        expect(card.getByText("standup")).toBeInTheDocument();
        expect(card.getByText("Suggested by Slack")).toBeInTheDocument();
        expect(card.getByText("45m")).toBeInTheDocument();
        expect(within(await mailCard()).getByText("Suggested by Mail")).toBeInTheDocument();
      });

      it("puts each before the recording card it precedes and the last one at the end", async () => {
        renderDay(withSuggestions(), withProject);
        await standupCard();
        const order = within(sheetOf())
          .getAllByLabelText(/^(Suggested: standup|review, |Suggested: Website, Website)/)
          .map((el) => el.getAttribute("aria-label")?.split(",")[0]);
        expect(order).toEqual(["Suggested: standup", "review", "Suggested: Website"]);
      });

      it("draws none when the day has no suggestions", async () => {
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        expect(screen.queryByRole("group", { name: /^Suggested:/ })).not.toBeInTheDocument();
        expect(screen.queryByRole("group", { name: "Suggestions to review" })).not.toBeInTheDocument();
      });

      it("approves with ✓, records on the shown day and offers Undo, without opening the editor", async () => {
        renderDay(withSuggestions(), withProject);
        await userEvent.click(within(await standupCard()).getByRole("button", { name: "Approve" }));
        expect(mocked.suggestionApprove).toHaveBeenCalledWith(DAY, standupIdea.id);
        expect(await screen.findByText("Recorded 45m for Website")).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
        expect(screen.queryByRole("form")).not.toBeInTheDocument();
      });

      it("shows the error when the suggestion is gone", async () => {
        mocked.suggestionApprove.mockRejectedValueOnce({ kind: "not_found", message: "Suggestion not found", code: null });
        renderDay(withSuggestions(), withProject);
        await userEvent.click(within(await standupCard()).getByRole("button", { name: "Approve" }));
        expect(await screen.findByRole("alert")).toHaveTextContent("Could not approve the suggestion: Suggestion not found");
      });

      it("dismisses with ✕ and offers Undo", async () => {
        renderDay(withSuggestions(), withProject);
        await userEvent.click(within(await standupCard()).getByRole("button", { name: "Dismiss" }));
        expect(mocked.suggestionsDismiss).toHaveBeenCalledWith(DAY, [standupIdea.id]);
        expect(await screen.findByText("Dismissed 1 suggestion")).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
      });
    });

    describe("the time column", () => {
      it("draws each suggestion as a hatched button in the recordings column", async () => {
        renderDay(withSuggestions(), withProject);
        const el = await screen.findByRole("button", { name: "Suggested: Website 09:00–09:45, by Slack" });
        expect(screen.getByLabelText("Recordings")).toContainElement(el);
        expect(within(el).getByText("Slack")).toBeInTheDocument();
      });

      it("opens the suggestion in the editor with a click", async () => {
        renderDay(withSuggestions(), withProject);
        await userEvent.click(await screen.findByRole("button", { name: "Suggested: Website 09:00–09:45, by Slack" }));
        expect(await editorForm("Edit suggestion")).toBeInTheDocument();
        expect(mocked.suggestionApprove).not.toHaveBeenCalled();
      });
    });

    describe("editing", () => {
      it("opens the editor in place of the card, prefilled with the note and project of the suggestion", async () => {
        renderDay(withSuggestions(), withProject);
        await userEvent.click(await standupCard());
        const form = await editorForm("Edit suggestion");
        expect(sheetOf()).toContainElement(form);
        expect(within(form).getByLabelText("Note")).toHaveValue("standup");
        expect(within(form).getByRole("combobox", { name: "Project" })).toHaveValue("Website");
        expectSpan(form, "09:00", "09:45");
        expect(within(form).getByText("Suggested by Slack. Approve records it.")).toBeInTheDocument();
        expect(screen.queryByRole("group", { name: /^Suggested: standup/ })).not.toBeInTheDocument();
      });

      it("approves the edited suggestion with Approve, sending the editor's input", async () => {
        renderDay(withSuggestions(), withProject);
        await userEvent.click(await standupCard());
        const form = within(await editorForm("Edit suggestion"));
        await userEvent.type(form.getByLabelText("Note"), " notes");
        await userEvent.click(form.getByRole("button", { name: "Approve" }));
        await waitFor(() =>
          expect(mocked.suggestionApproveEdited).toHaveBeenCalledWith(DAY, standupIdea.id, { projectId: "p-site", spans: [{ start: at("09:00"), end: at("09:45") }], note: "standup notes", tagId: null }, "cancel"),
        );
        expect(mocked.recordingCreate).not.toHaveBeenCalled();
        await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
      });

      it("keeps the suggestion when the editor is cancelled", async () => {
        renderDay(withSuggestions(), withProject);
        await userEvent.click(await standupCard());
        await editorForm("Edit suggestion");
        await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
        expect(await standupCard()).toBeInTheDocument();
        expect(mocked.suggestionApproveEdited).not.toHaveBeenCalled();
      });
    });

    describe("the review line", () => {
      const reviewLine = () => screen.findByRole("group", { name: "Suggestions to review" });

      it("counts the suggestions of the day", async () => {
        renderDay(withSuggestions(), withProject);
        expect(within(await reviewLine()).getByText("2 suggestions")).toBeInTheDocument();
      });

      it("says 1 suggestion for a single one", async () => {
        renderDay(dayView({ lanes: [lane({ blocks: [docs] })], suggestions: [mailIdea] }), withProject);
        expect(within(await reviewLine()).getByText("1 suggestion")).toBeInTheDocument();
      });

      it("approves every shown suggestion with Approve all", async () => {
        mocked.suggestionsApprove.mockResolvedValueOnce([recording({ id: "r1" }), recording({ id: "r2" })]);
        renderDay(withSuggestions(), withProject);
        await userEvent.click(within(await reviewLine()).getByRole("button", { name: "Approve all" }));
        expect(mocked.suggestionsApprove).toHaveBeenCalledWith(DAY, [standupIdea.id, mailIdea.id]);
        expect(await screen.findByText("Recorded 2 suggestions")).toBeInTheDocument();
      });

      it("dismisses every shown suggestion with Dismiss all", async () => {
        mocked.suggestionsDismiss.mockResolvedValueOnce(2);
        renderDay(withSuggestions(), withProject);
        await userEvent.click(within(await reviewLine()).getByRole("button", { name: "Dismiss all" }));
        expect(mocked.suggestionsDismiss).toHaveBeenCalledWith(DAY, [standupIdea.id, mailIdea.id]);
        expect(await screen.findByText("Dismissed 2 suggestions")).toBeInTheDocument();
      });
    });

    describe("keys on a focused suggestion", () => {
      it("approves it with ↵", async () => {
        renderDay(withSuggestions(), withProject);
        await focusCard(standupCard());
        await userEvent.keyboard("{Enter}");
        await vi.waitFor(() => expect(mocked.suggestionApprove).toHaveBeenCalledWith(DAY, standupIdea.id));
        expect(screen.queryByRole("form")).not.toBeInTheDocument();
      });

      it("opens it in the editor with E", async () => {
        renderDay(withSuggestions(), withProject);
        await focusCard(standupCard());
        await userEvent.keyboard("e");
        expect(await editorForm("Edit suggestion")).toBeInTheDocument();
      });

      it("dismisses it with ⌫", async () => {
        renderDay(withSuggestions(), withProject);
        await focusCard(standupCard());
        await userEvent.keyboard("{Backspace}");
        await vi.waitFor(() => expect(mocked.suggestionsDismiss).toHaveBeenCalledWith(DAY, [standupIdea.id]));
      });

      it("moves the focus to the next suggestion once it is approved", async () => {
        renderDay(withSuggestions(), withProject);
        await focusCard(standupCard());
        await userEvent.keyboard("{Enter}");
        await vi.waitFor(() => expect(useStore.getState().focusedSuggestion).toBe(mailIdea.id));
      });

      it("moves the focus to the next suggestion once it is dismissed", async () => {
        renderDay(withSuggestions(), withProject);
        await focusCard(standupCard());
        await userEvent.keyboard("{Backspace}");
        await vi.waitFor(() => expect(useStore.getState().focusedSuggestion).toBe(mailIdea.id));
      });

      it("reaches suggestions and recordings alike with ]", async () => {
        renderDay(withSuggestions(), withProject);
        await standupCard();
        await userEvent.keyboard("]");
        expect(useStore.getState().focusedSuggestion).toBe(standupIdea.id);
        await userEvent.keyboard("]");
        expect(useStore.getState().focusedRecording).toBe("c1");
        await userEvent.keyboard("]");
        expect(useStore.getState().focusedSuggestion).toBe(mailIdea.id);
      });

      it("clears the focus with Escape", async () => {
        renderDay(withSuggestions(), withProject);
        await focusCard(standupCard());
        await userEvent.keyboard("{Escape}");
        expect(useStore.getState().focusedSuggestion).toBeNull();
      });
    });
  });

  describe("keyboard", () => {
    const review = recordingBlock({ id: "r2", projectId: "p-site", projectName: "Website", note: "review", start: at("13:00"), end: at("13:30") });
    const twoRecordings = () => withTimesheet(dayView({ lanes: [lane({ blocks: [docs, todo] })], recordings: [review, meeting] }));
    const focusedIn = (labels: string[]) => labels.filter((label) => screen.queryByLabelText(label)?.classList.contains("focused"));
    const focusedBlocks = () => focusedIn(["Safari: Docs", "Notes: Todo", "Planner: Standup"]);
    const focusedRecordings = () => focusedIn(["Website: standup", "Website: review"]);

    describe("V selects the focused block", () => {
      it("shows the block's span as the selection and opens the editor with it on ↵", async () => {
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        await userEvent.keyboard("jv");
        expect(await screen.findByLabelText("Selection 09:00–09:30")).toBeInTheDocument();
        expect(screen.queryByRole("form")).not.toBeInTheDocument();
        await userEvent.keyboard("{Enter}");
        expectSpan(await editorForm("New recording"), "09:00", "09:30");
      });

      it("does nothing without a focused block", async () => {
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        await userEvent.keyboard("v{Enter}");
        expect(screen.queryByLabelText(/^Selection/)).not.toBeInTheDocument();
        expect(screen.queryByRole("form")).not.toBeInTheDocument();
      });
    });

    describe("⇧J and ⇧K extend the selection", () => {
      it("grows the selection over the next block and moves the focus to it", async () => {
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        await userEvent.keyboard("j{Shift>}j{/Shift}");
        expect(await screen.findByLabelText("Selection 09:00–10:30")).toBeInTheDocument();
        expect(focusedBlocks()).toEqual(["Notes: Todo"]);
        await userEvent.keyboard("{Enter}");
        expectSpan(await editorForm("New recording"), "09:00", "10:30");
      });

      it("grows the selection over the previous block and moves the focus to it", async () => {
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        await userEvent.keyboard("jj{Shift>}k{/Shift}");
        expect(await screen.findByLabelText("Selection 09:00–10:30")).toBeInTheDocument();
        expect(focusedBlocks()).toEqual(["Safari: Docs"]);
      });

      it("selects the first block when nothing is focused", async () => {
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        await userEvent.keyboard("{Shift>}j{/Shift}");
        expect(await screen.findByLabelText("Selection 09:00–09:30")).toBeInTheDocument();
        expect(focusedBlocks()).toEqual(["Safari: Docs"]);
      });

      it("clears the focused recording", async () => {
        renderDay(recordedDay(), withProject);
        await blockEl("Website: standup");
        await userEvent.keyboard("]");
        await userEvent.keyboard("{Shift>}j{/Shift}{Backspace}");
        expect(await screen.findByLabelText(/^Selection/)).toBeInTheDocument();
        expect(mocked.recordingDelete).not.toHaveBeenCalled();
      });
    });

    describe("] and [ focus recordings", () => {
      it("] focuses the first recording by start, then the next, and stops at the last", async () => {
        renderDay(twoRecordings(), withProject);
        await blockEl("Website: standup");
        await userEvent.keyboard("]");
        expect(focusedRecordings()).toEqual(["Website: standup"]);
        await userEvent.keyboard("]");
        expect(focusedRecordings()).toEqual(["Website: review"]);
        await userEvent.keyboard("]");
        expect(focusedRecordings()).toEqual(["Website: review"]);
      });

      it("[ goes back to the previous recording and stops at the first", async () => {
        renderDay(twoRecordings(), withProject);
        await blockEl("Website: standup");
        await userEvent.keyboard("]][[");
        expect(focusedRecordings()).toEqual(["Website: standup"]);
        await userEvent.keyboard("[[");
        expect(focusedRecordings()).toEqual(["Website: standup"]);
      });

      it("[ starts from the last recording", async () => {
        renderDay(twoRecordings(), withProject);
        await blockEl("Website: standup");
        await userEvent.keyboard("[[");
        expect(focusedRecordings()).toEqual(["Website: review"]);
      });

      it("↵ opens the editor of the focused recording", async () => {
        mocked.recordingsList.mockResolvedValue([meetingRow]);
        renderDay(twoRecordings(), withProject);
        await blockEl("Website: standup");
        await userEvent.keyboard("]{Enter}");
        expectSpan(await editorForm("Edit recording"), "11:00", "11:45");
      });

      it("clears the selection and the block focus, so ↵ edits the recording instead of creating one", async () => {
        mocked.recordingsList.mockResolvedValue([meetingRow]);
        renderDay(twoRecordings(), withProject);
        await blockEl("Safari: Docs");
        await userEvent.keyboard("jv");
        await screen.findByLabelText(/^Selection/);
        await userEvent.keyboard("]");
        expect(screen.queryByLabelText(/^Selection/)).not.toBeInTheDocument();
        expect(focusedBlocks()).toEqual([]);
        await userEvent.keyboard("{Enter}");
        expect(await editorForm("Edit recording")).toBeInTheDocument();
        expect(screen.queryByRole("form", { name: "New recording" })).not.toBeInTheDocument();
      });

      it("does nothing on a day without recordings", async () => {
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        await userEvent.keyboard("]{Enter}");
        expect(screen.queryByRole("form")).not.toBeInTheDocument();
      });
    });

    describe("⌥← and ⌥→ move the focused recording to another day", () => {
      it.each([
        ["⌥← moves it to the previous day", "{Alt>}{ArrowLeft}{/Alt}", "recordingMove", "recordingCopy", "2026-03-09"],
        ["⌥→ moves it to the next day", "{Alt>}{ArrowRight}{/Alt}", "recordingMove", "recordingCopy", "2026-03-11"],
        ["⌥⇧← copies it to the previous day", "{Alt>}{Shift>}{ArrowLeft}{/Shift}{/Alt}", "recordingCopy", "recordingMove", "2026-03-09"],
        ["⌥⇧→ copies it to the next day", "{Alt>}{Shift>}{ArrowRight}{/Shift}{/Alt}", "recordingCopy", "recordingMove", "2026-03-11"],
      ] as const)("%s", async (_name, keys, call, other, day) => {
        renderDay(recordedDay(), withProject);
        await blockEl("Website: standup");
        await userEvent.keyboard("]");
        await userEvent.keyboard(keys);
        await vi.waitFor(() => expect(mocked[call]).toHaveBeenCalledWith("r1", null, day, "cancel"));
        expect(mocked[other]).not.toHaveBeenCalled();
      });

      it.each([
        ["⌥←", "{Alt>}{ArrowLeft}{/Alt}"],
        ["⌥⇧→", "{Alt>}{Shift>}{ArrowRight}{/Shift}{/Alt}"],
      ])("%s does nothing without a focused recording", async (_name, keys) => {
        renderDay(recordedDay(), withProject);
        await blockEl("Website: standup");
        await userEvent.keyboard(keys);
        expect(mocked.recordingMove).not.toHaveBeenCalled();
        expect(mocked.recordingCopy).not.toHaveBeenCalled();
      });
    });

    describe("the block menu opened with ⌃↵", () => {
      const openBlockMenu = async (keys: string) => {
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        await userEvent.keyboard(`${keys}{Control>}{Enter}{/Control}`);
        await screen.findByRole("menu");
      };

      it("closes with Escape and leaves the block focused", async () => {
        await openBlockMenu("j");
        await userEvent.keyboard("{Escape}");
        expect(screen.queryByRole("menu")).not.toBeInTheDocument();
        expect(focusedBlocks()).toEqual(["Safari: Docs"]);
      });

      it("runs its focused item on ↵ rather than a command, even while a selection exists", async () => {
        await openBlockMenu("jv");
        await userEvent.keyboard("{Enter}");
        expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Safari", { groupByApp: false });
        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      });
    });

    describe("↵ and Space on a focused button", () => {
      it.each([
        ["↵", "{Enter}"],
        ["Space", " "],
      ])("%s activates the button instead of opening the editor or the details", async (_name, keys) => {
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        await userEvent.keyboard("jv");
        await screen.findByLabelText(/^Selection/);
        screen.getByRole("button", { name: "More day actions" }).focus();
        await userEvent.keyboard(keys);
        expect(await screen.findByRole("menu")).toBeInTheDocument();
        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
        expect(screen.queryByRole("form")).not.toBeInTheDocument();
      });
    });

    describe("the shortcuts overview", () => {
      it("lists the bound day commands with their keys", async () => {
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        act(() => {
          runCommand("shortcuts.show");
        });
        const day = within(within(await screen.findByRole("dialog", { name: "Keyboard shortcuts" })).getByRole("region", { name: "Day" }));
        for (const [title, keys] of [
          ["Select activity", "V"],
          ["Extend selection to the next activity", "⇧J"],
          ["Extend selection to the previous activity", "⇧K"],
          ["Next recording or suggestion", "]"],
          ["Previous recording or suggestion", "["],
          ["Move recording to the previous day", "⌥←"],
          ["Move recording to the next day", "⌥→"],
          ["Copy recording to the previous day", "⌥⇧←"],
          ["Copy recording to the next day", "⌥⇧→"],
        ]) {
          expect(within(day.getByText(title).parentElement as HTMLElement).getByText(keys)).toBeInTheDocument();
        }
      });
    });
  });

  describe("making a rule by dragging", () => {
    let over: Element | null;

    beforeEach(() => {
      over = null;
      document.elementFromPoint = vi.fn(() => over);
    });

    afterEach(() => {
      delete (document as { elementFromPoint?: unknown }).elementFromPoint;
    });

    const strip = () => screen.findByRole("list", { name: "Drop on a project to make a rule" });

    /** ⌥-presses `label` and moves the pointer; the strip of projects is up afterwards. */
    async function altDrag(label: string) {
      const el = await blockEl(label);
      const lanes = lanesEl();
      fireEvent.pointerDown(el, { button: 0, altKey: true, clientX: 300, clientY: 200 });
      fireEvent.pointerMove(lanes, { clientX: 300, clientY: 230 });
      const projects = within(await strip());
      return { lanes, target: projects.getAllByRole("listitem")[0] };
    }

    const sheet = () => within(screen.getByRole("dialog", { name: "New rule" }));

    it("shows the recent projects as drop targets while an activity block is dragged with ⌥", async () => {
      renderDay(busyDay(), withProject);
      await altDrag("Safari: Docs");
      expect(within(await strip()).getAllByRole("listitem").map((i) => i.textContent)).toEqual(["Website"]);
    });

    it("opens the rule sheet for the app and the domain when a URL block is dropped on a project", async () => {
      renderDay(busyDay(), withProject);
      const { lanes, target } = await altDrag("Safari: Docs");
      over = target;
      fireEvent.pointerUp(lanes, { clientX: 300, clientY: 230 });
      expect(await screen.findByRole("dialog", { name: "New rule" })).toBeInTheDocument();
      expect(sheet().getByLabelText("Name")).toHaveValue("Safari → Website");
      expect(sheet().getByRole("combobox", { name: "Project" })).toHaveValue("Website");
      expect(sheet().getByLabelText("Condition 1 value")).toHaveValue("Safari");
      expect(sheet().getByLabelText("Condition 2 field")).toHaveValue("domain");
      expect(sheet().getByLabelText("Condition 2 value")).toHaveValue("example.com");
      expect(screen.queryByRole("list", { name: "Drop on a project to make a rule" })).not.toBeInTheDocument();
    });

    it("opens the rule sheet for the app and the title when a block without a URL is dropped on a project", async () => {
      renderDay(busyDay(), withProject);
      const { lanes, target } = await altDrag("Notes: Todo");
      over = target;
      fireEvent.pointerUp(lanes, { clientX: 300, clientY: 230 });
      expect(await screen.findByRole("dialog", { name: "New rule" })).toBeInTheDocument();
      expect(sheet().getByLabelText("Name")).toHaveValue("Notes → Website");
      expect(sheet().getByLabelText("Condition 2 field")).toHaveValue("title");
      expect(sheet().getByLabelText("Condition 2 comparison")).toHaveValue("contains");
      expect(sheet().getByLabelText("Condition 2 value")).toHaveValue("Todo");
    });

    it("saves nothing until the sheet's Save", async () => {
      renderDay(busyDay(), withProject);
      const { lanes, target } = await altDrag("Notes: Todo");
      over = target;
      fireEvent.pointerUp(lanes, { clientX: 300, clientY: 230 });
      await screen.findByRole("dialog", { name: "New rule" });
      expect(mocked.ruleSave).not.toHaveBeenCalled();
      await userEvent.click(sheet().getByRole("button", { name: "Save" }));
      expect(mocked.ruleSave).toHaveBeenCalledWith(null, expect.objectContaining({ name: "Notes → Website", projectId: "p-site" }));
    });

    it("opens no sheet when released away from the projects", async () => {
      renderDay(busyDay(), withProject);
      const { lanes } = await altDrag("Safari: Docs");
      fireEvent.pointerUp(lanes, { clientX: 300, clientY: 230 });
      await vi.waitFor(() => expect(screen.queryByRole("list", { name: "Drop on a project to make a rule" })).not.toBeInTheDocument());
      expect(screen.queryByRole("dialog", { name: "New rule" })).not.toBeInTheDocument();
    });

    it("opens no sheet for a press that was not moved", async () => {
      renderDay(busyDay(), withProject);
      const el = await blockEl("Safari: Docs");
      const lanes = lanesEl();
      fireEvent.pointerDown(el, { button: 0, altKey: true, clientX: 300, clientY: 200 });
      fireEvent.pointerUp(lanes, { clientX: 300, clientY: 200 });
      expect(screen.queryByRole("dialog", { name: "New rule" })).not.toBeInTheDocument();
    });

    it("shows no drop strip when the block is dragged without ⌥", async () => {
      renderDay(busyDay(), withProject);
      const el = await blockEl("Safari: Docs");
      fireEvent.pointerDown(el, { button: 0, clientX: 300, clientY: 200 });
      fireEvent.pointerMove(lanesEl(), { clientX: 300, clientY: 230 });
      expect(screen.queryByRole("list", { name: "Drop on a project to make a rule" })).not.toBeInTheDocument();
    });
  });
});
