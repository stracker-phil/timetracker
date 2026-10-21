import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type DayView } from "../../api";
import { handleKeydown, installCommands, runCommand } from "../../commands";
import { today } from "../../lib/day";
import { resetStore, useStore } from "../../state/store";
import { trackerStatus } from "../../test/data";
import { appLane, at, away, block, dayBounds, DAY, dayView, lane, recordingBlock, sourceLane, summaryLane } from "../../test/dayFixtures";
import { projectItem, projectsStore } from "../../test/projectFixtures";
import { recording } from "../../test/recordingFixtures";
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
const recordedDay = () => dayView({ lanes: [lane({ blocks: [docs, todo] })], recordings: [meeting] });
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
      expect(screen.getAllByRole("columnheader").map((h) => h.textContent)).toEqual(["Activity", "planner"]);
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

    it("labels an away span with its reason and length", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], away: [away({ reason: "idle", start: at("12:00"), end: at("12:30") })] }));
      expect(await screen.findByText("Away · idle 30 min")).toBeInTheDocument();
    });

    it("leaves a very short away span unlabeled", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], away: [away({ start: at("12:00"), end: at("12:05") })] }));
      await blockEl("Safari: Docs");
      expect(screen.queryByText(/^Away/)).not.toBeInTheDocument();
    });
  });

  describe("header", () => {
    it("shows the totals of the day", async () => {
      const totals = { recordedMs: 0, trackedMs: (6 * 60 + 12) * 60_000, awayMs: (60 + 5) * 60_000 };
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], totals }));
      expect(await screen.findByText("recorded 0:00 · tracked 6:12 · away 1:05")).toBeInTheDocument();
    });

    it("shows the recorded time of the day", async () => {
      const totals = { recordedMs: 90 * MINUTE_MS, trackedMs: 0, awayMs: 0 };
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], recordings: [meeting], totals }));
      expect(await screen.findByText("recorded 1:30 · tracked 0:00 · away 0:00")).toBeInTheDocument();
    });

    it("steps to the previous and next day with its buttons", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.click(screen.getByRole("button", { name: "Next day" }));
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenLastCalledWith("2026-03-11"));
      await userEvent.click(screen.getByRole("button", { name: "Previous day" }));
      await userEvent.click(screen.getByRole("button", { name: "Previous day" }));
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenLastCalledWith("2026-03-09"));
    });

    it("disables Today on today and enables it on another day", async () => {
      renderDay(dayView({ day: today() }));
      expect(screen.getByRole("button", { name: "Today" })).toBeDisabled();
      act(() => useStore.getState().prevDay());
      expect(screen.getByRole("button", { name: "Today" })).toBeEnabled();
    });
  });

  describe("block labels", () => {
    it("shows the app and the label of a block", async () => {
      const doc = block({ id: "a6", app: "Preview", title: "Report.pdf — Page 2", label: "Report.pdf" });
      renderDay(dayView({ lanes: [lane({ blocks: [doc] })] }));
      const el = await blockEl("Preview: Report.pdf");
      expect(within(el).getByText("Preview")).toBeInTheDocument();
      expect(within(el).getByText("Report.pdf")).toBeInTheDocument();
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

  describe("timeline settings", () => {
    const mail = { appKey: "com.apple.Mail", app: "Mail" };

    it("opens from the header and lists the hidden apps", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], hiddenApps: [mail] }));
      await blockEl("Safari: Docs");
      await userEvent.click(screen.getByRole("button", { name: "Timeline settings" }));
      const dialog = within(await screen.findByRole("dialog", { name: "Timeline settings" }));
      expect(dialog.getByRole("heading", { name: "Hidden apps: 1" })).toBeInTheDocument();
      expect(dialog.getByText("Mail")).toBeInTheDocument();
    });

    it("says no app is hidden when none is", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.click(screen.getByRole("button", { name: "Timeline settings" }));
      expect(await screen.findByRole("heading", { name: "Hidden apps: 0" })).toBeInTheDocument();
      expect(screen.getByText("No hidden apps")).toBeInTheDocument();
    });

    it("shows a hidden app again with Unhide", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [docs] })], hiddenApps: [mail] }));
      await blockEl("Safari: Docs");
      await userEvent.click(screen.getByRole("button", { name: "Timeline settings" }));
      await userEvent.click(await screen.findByRole("button", { name: "Unhide" }));
      expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Mail", { hidden: false });
    });

    it("opens with the day.settings command", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      act(() => {
        runCommand("day.settings");
      });
      expect(await screen.findByRole("dialog", { name: "Timeline settings" })).toBeInTheDocument();
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
  });

  describe("day shortcuts", () => {
    it("loads the next day with ⌘→ and the previous with ⌘←", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("{Meta>}{ArrowRight}{/Meta}");
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenLastCalledWith("2026-03-11"));
      await userEvent.keyboard("{Meta>}{ArrowLeft}{ArrowLeft}{/Meta}");
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenLastCalledWith("2026-03-09"));
    });

    it("returns to today with T", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("t");
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenLastCalledWith(today()));
      expect(screen.getByRole("button", { name: "Today" })).toBeDisabled();
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
      expect(card.getByText("09:12–09:40 · 28 min")).toBeInTheDocument();
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
      expect(screen.getByText("10:00–11:00 · 1 h 00 min · planner")).toBeInTheDocument();
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

    it("offers the app settings, hide and copy for a tracker block with a url", async () => {
      renderDay(busyDay());
      const menu = within(await openMenu("Safari: Docs"));
      expect(menu.getAllByRole("menuitem").map((i) => i.textContent)).toEqual(["Hide Safari", "Copy title", "Copy URL"]);
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

    it("offers only copying for an external block", async () => {
      renderDay(busyDay());
      const menu = within(await openMenu("Planner: Standup"));
      expect(menu.getAllByRole("menuitem").map((i) => i.textContent)).toEqual(["Copy title"]);
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
    const editorDialog = () => screen.findByRole("dialog", { name: "New recording" });

    it("selects the span of a clicked block and shows it", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      expect(await screen.findByLabelText("Selection")).toHaveTextContent("09:00–09:30");
    });

    it("opens the editor with the span of the clicked block on Enter", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      await userEvent.keyboard("{Enter}");
      const dialog = within(await editorDialog());
      expect(dialog.getByLabelText("From")).toHaveValue("09:00");
      expect(dialog.getByLabelText("To")).toHaveValue("09:30");
    });

    it("extends the selection from the earliest start to the latest end with a shift-click", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Notes: Todo"));
      const first = screen.getByLabelText("Safari: Docs");
      fireEvent.pointerDown(first, { button: 0, shiftKey: true });
      fireEvent.pointerUp(first, { shiftKey: true });
      await screen.findByText("09:00–10:30");
      await userEvent.keyboard("{Enter}");
      const dialog = within(await editorDialog());
      expect(dialog.getByLabelText("From")).toHaveValue("09:00");
      expect(dialog.getByLabelText("To")).toHaveValue("10:30");
    });

    it("opens the editor with the extended span even before Rust answered the snap", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Notes: Todo"));
      await screen.findByText("10:00–10:30");
      mocked.recordingSnap.mockReturnValue(new Promise(() => {}));
      const first = screen.getByLabelText("Safari: Docs");
      fireEvent.pointerDown(first, { button: 0, shiftKey: true });
      fireEvent.pointerUp(first, { shiftKey: true });
      await userEvent.keyboard("{Enter}");
      const dialog = within(await editorDialog());
      expect(dialog.getByLabelText("From")).toHaveValue("09:00");
      expect(dialog.getByLabelText("To")).toHaveValue("10:30");
    });

    it("replaces the selection when another block is clicked without shift", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      await userEvent.click(screen.getByLabelText("Notes: Todo"));
      expect(await screen.findByText("10:00–10:30")).toBeInTheDocument();
    });

    it("clears the selection with Escape", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      await screen.findByLabelText("Selection");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByLabelText("Selection")).not.toBeInTheDocument();
    });

    it("clears the selection when empty space is clicked", async () => {
      renderDay(busyDay());
      await userEvent.click(await blockEl("Safari: Docs"));
      await screen.findByLabelText("Selection");
      await userEvent.click(lanesEl());
      expect(screen.queryByLabelText("Selection")).not.toBeInTheDocument();
    });

    describe("dragging on the lanes", () => {
      async function dragTo(clientX: number) {
        mocked.recordingSnap.mockResolvedValue({ start: at("09:00"), end: at("10:30") });
        renderDay(busyDay());
        await blockEl("Safari: Docs");
        vi.spyOn(screen.getByLabelText("Recordings"), "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 100, 3600));
        fireEvent.pointerDown(lanesEl(), { button: 0, clientX: 300, clientY: px("09:02") });
        fireEvent.pointerMove(lanesEl(), { clientX: 300, clientY: px("10:28") });
        await screen.findByText("09:00–10:30");
        fireEvent.pointerUp(lanesEl(), { clientX, clientY: px("10:28") });
      }

      it("shows the dragged span with the times Rust snaps it to", async () => {
        await dragTo(300);
        expect(screen.getByLabelText("Selection")).toHaveTextContent("09:00–10:30");
        expect(mocked.recordingSnap).toHaveBeenCalledWith(at("09:02"), at("10:28"));
      });

      it("opens the editor with the snapped span when released over the recordings column", async () => {
        await dragTo(50);
        const dialog = within(await editorDialog());
        expect(dialog.getByLabelText("From")).toHaveValue("09:00");
        expect(dialog.getByLabelText("To")).toHaveValue("10:30");
      });

      it("keeps the selection and opens no editor when released elsewhere", async () => {
        await dragTo(300);
        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
        expect(screen.getByLabelText("Selection")).toBeInTheDocument();
      });
    });
  });

  describe("recordings", () => {
    const recordingEl = () => screen.findByLabelText("Website: standup");
    const grip = (el: HTMLElement, which: "start" | "end") => within(el).getByLabelText(`Drag to change the ${which}`);
    const shiftedBy = (minutes: { start: number; end: number }) => ({ start: meeting.start + minutes.start * MINUTE_MS, end: meeting.end + minutes.end * MINUTE_MS });

    it("shows a recording as project and note in the recordings column", async () => {
      renderDay(recordedDay());
      expect(within(await recordingEl()).getByText("Website")).toBeInTheDocument();
      expect(screen.getByLabelText("Recordings")).toContainElement(screen.getByLabelText("Website: standup"));
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

    it("opens the editor on the recording with a double-click", async () => {
      mocked.recordingsList.mockResolvedValue([meetingRow]);
      renderDay(recordedDay(), withProject);
      await userEvent.dblClick(await recordingEl());
      expect(await screen.findByRole("dialog", { name: "Edit recording" })).toBeInTheDocument();
    });

    it("opens the editor on the focused recording with Enter", async () => {
      mocked.recordingsList.mockResolvedValue([meetingRow]);
      renderDay(recordedDay(), withProject);
      await userEvent.click(await recordingEl());
      await userEvent.keyboard("{Enter}");
      expect(await screen.findByRole("dialog", { name: "Edit recording" })).toBeInTheDocument();
    });

    describe("dragging", () => {
      it.each([
        ["its body moves it", null, 25, "11:10", "11:55"],
        ["its top edge changes the start", "start", -25, "10:50", "11:45"],
        ["its bottom edge changes the end", "end", 25, "11:00", "11:55"],
      ] as const)("%s", async (_name, edge, dy, start, end) => {
        renderDay(recordedDay(), withProject);
        const el = await recordingEl();
        const handle = edge ? grip(el, edge) : el;
        fireEvent.pointerDown(handle, { button: 0, clientY: 1000 });
        fireEvent.pointerMove(handle, { clientY: 1000 + dy });
        fireEvent.pointerUp(handle, { clientY: 1000 + dy });
        await vi.waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", { start: at(start), end: at(end) }, "cancel"));
        expect(await screen.findByText("Recording changed")).toBeInTheDocument();
      });

      it("changes nothing when the pointer is released where it was pressed", async () => {
        renderDay(recordedDay(), withProject);
        const handle = grip(await recordingEl(), "end");
        fireEvent.pointerDown(handle, { button: 0, clientY: 1000 });
        fireEvent.pointerUp(handle, { clientY: 1000 });
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      });

      it("does not move a recording that continues on another day", async () => {
        renderDay(dayView({ recordings: [{ ...meeting, clippedEnd: true }] }), withProject);
        const el = await recordingEl();
        fireEvent.pointerDown(el, { button: 0, clientY: 1000 });
        fireEvent.pointerMove(el, { clientY: 1025 });
        fireEvent.pointerUp(el, { clientY: 1025 });
        expect(within(el).queryByLabelText(/^Drag to change/)).not.toBeInTheDocument();
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      });

      it("shows an error toast when Rust refuses the new times", async () => {
        mocked.recordingUpdate.mockRejectedValueOnce({ kind: "invalid_input", message: "`end`: too long", code: null, field: { path: "end", message: "Too long" } });
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
        ["Alt+↑ moves it one snap step earlier", "{Alt>}{ArrowUp}{/Alt}", { start: -5, end: -5 }],
        ["Alt+↓ moves it one snap step later", "{Alt>}{ArrowDown}{/Alt}", { start: 5, end: 5 }],
        ["Alt+Shift+↓ extends its end by one step", "{Alt>}{Shift>}{ArrowDown}{/Shift}{/Alt}", { start: 0, end: 5 }],
        ["Alt+Shift+↑ shortens its end by one step", "{Alt>}{Shift>}{ArrowUp}{/Shift}{/Alt}", { start: 0, end: -5 }],
      ])("%s", async (_name, keys, minutes) => {
        renderDay(recordedDay(), withProject);
        await userEvent.click(await recordingEl());
        await userEvent.keyboard(keys);
        await vi.waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", shiftedBy(minutes), "cancel"));
      });

      it("steps by the snap interval of the settings", async () => {
        renderDay(recordedDay(), { ...withProject, settings: settings({ timeline: { ...settings().timeline, snapMinutes: 15 } }) });
        await userEvent.click(await recordingEl());
        await userEvent.keyboard("{Alt>}{ArrowDown}{/Alt}");
        await vi.waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", shiftedBy({ start: 15, end: 15 }), "cancel"));
      });

      it("does not nudge a recording that continues on another day", async () => {
        renderDay(dayView({ recordings: [{ ...meeting, clippedStart: true }] }), withProject);
        await userEvent.click(await recordingEl());
        await userEvent.keyboard("{Alt>}{ArrowDown}{/Alt}");
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      });

      it("deletes the focused recording with Backspace and offers Undo", async () => {
        renderDay(recordedDay(), withProject);
        await userEvent.click(await recordingEl());
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
        await userEvent.click(await recordingEl());
        await userEvent.keyboard("{Escape}{Backspace}");
        expect(mocked.recordingDelete).not.toHaveBeenCalled();
      });
    });
  });
});
