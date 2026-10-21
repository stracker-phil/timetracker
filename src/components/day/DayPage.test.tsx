import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type DayView } from "../../api";
import { handleKeydown, installCommands } from "../../commands";
import { today } from "../../lib/day";
import { resetStore, useStore } from "../../state/store";
import { trackerStatus } from "../../test/data";
import { appLane, at, away, block, DAY, dayView, lane, sourceLane } from "../../test/dayFixtures";
import { PermissionDialog } from "../PermissionDialog";
import { Toasts } from "../Toasts";
import { DayPage } from "./DayPage";

const mocked = vi.mocked(api, true);

const VIEWPORT_PX = 600;

const docs = block({ id: "a1", app: "Safari", title: "Docs", start: at("09:00"), end: at("09:30"), url: "https://example.com/docs" });
const todo = block({ id: "a2", app: "Notes", title: "Todo", bundleId: "com.apple.Notes", start: at("10:00"), end: at("10:30") });
const standup = block({ id: "a3", app: "Planner", title: "Standup", bundleId: null, start: at("10:00"), end: at("11:00") });
const busyDay = () => dayView({ lanes: [lane({ blocks: [docs, todo] }), sourceLane("planner", [standup])] });

function renderDay(view: DayView, patch: Parameters<typeof resetStore>[0] = {}) {
  mocked.dayView.mockResolvedValue(view);
  resetStore({ day: view.day, permissionDialogHidden: true, ...patch });
  installCommands();
  return render(
    <>
      <DayPage />
      <Toasts />
      <PermissionDialog />
    </>,
  );
}

const blockEl = (name: string) => screen.findByLabelText(name);
const scrollBox = () => screen.getByRole("row").parentElement as HTMLElement;

describe("DayPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

    it("opens the card of the focused block with Enter and closes it with another Enter", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("j{Enter}");
      expect(screen.getByRole("dialog", { name: "Safari details" })).toBeInTheDocument();
      await userEvent.keyboard("{Enter}");
      expect(screen.queryByRole("dialog", { name: "Safari details" })).not.toBeInTheDocument();
    });

    it("does nothing on Enter without a focused block", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("{Enter}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("opens the block menu with Ctrl+Enter", async () => {
      renderDay(busyDay());
      await blockEl("Safari: Docs");
      await userEvent.keyboard("j{Control>}{Enter}{/Control}");
      expect(await screen.findByRole("menuitem", { name: "Own lane for Safari" })).toBeInTheDocument();
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

    it("offers own lane, hide and copy for a tracker block with a bundle id and url", async () => {
      renderDay(busyDay());
      const menu = within(await openMenu("Safari: Docs"));
      expect(menu.getAllByRole("menuitem").map((i) => i.textContent)).toEqual(["Own lane for Safari", "Hide Safari", "Copy title", "Copy URL"]);
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
      await userEvent.click(within(await openMenu("Safari: Docs")).getByRole("menuitem", { name: "Own lane for Safari" }));
      expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Safari", { ownLane: true });
    });

    it("moves an app lane's block back to the activity lane", async () => {
      renderDay(dayView({ lanes: [lane({ blocks: [todo] }), appLane("Safari", "com.apple.Safari", [docs])] }));
      const menu = within(await openMenu("Safari: Docs"));
      expect(menu.queryByRole("menuitem", { name: "Own lane for Safari" })).not.toBeInTheDocument();
      await userEvent.click(menu.getByRole("menuitem", { name: "Back to activity lane" }));
      expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Safari", { ownLane: false });
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
});
