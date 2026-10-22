import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./api", async (orig) => ({ ...(await orig<object>()), api: (await import("./test/apiMock")).createApiMock() }));

import App from "./App";
import { api, EMPTY_UI_STATE } from "./api";
import { today } from "./lib/day";
import { resetStore, useStore, type View } from "./state/store";
import { activityRow, trackerStatus } from "./test/data";
import { miniMonth } from "./test/calendarFixtures";
import { dayView } from "./test/dayFixtures";
import { projectItem, tag } from "./test/projectFixtures";
import { savedReport } from "./test/reportFixtures";
import { rule } from "./test/ruleFixtures";
import { runningStatus } from "./test/timerFixtures";

const mocked = vi.mocked(api, true);

/** Renders the app and waits until the settings arrived and the commands are installed. */
async function renderApp(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore(patch);
  render(<App />);
  await vi.waitFor(() => expect(useStore.getState().settings).not.toBeNull());
  await screen.findByRole("navigation", { name: "Views" });
}

/** The Day, Week, Month and Reports views are regions, the Projects, Tags and Rules pages have a level-1 heading. */
const mainHeading = (name: string) => (["Day", "Week", "Month", "Reports"].includes(name) ? screen.getByRole("region", { name }) : screen.getByRole("heading", { level: 1, name }));
const sidebarEntry = (name: string) => within(screen.getByRole("navigation", { name: "Views" })).getByRole("button", { name: new RegExp(`^${name}`) });
const sidebar = () => within(screen.getByRole("complementary", { name: "Sidebar" }));

describe("App", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("shell", () => {
    it("shows the status bar and the Day view without a global title bar", async () => {
      await renderApp();
      expect(screen.queryByText("TimeTracker")).not.toBeInTheDocument();
      expect(await screen.findByText("Nothing tracked on this day")).toBeInTheDocument();
      expect(mainHeading("Day")).toBeInTheDocument();
    });

    it.each(["Hours", "Reports", "Projects", "Tags", "Rules"])("lists %s in the sidebar", async (name) => {
      await renderApp();
      expect(sidebarEntry(name)).toBeInTheDocument();
    });

    it("leaves Day, Week and Month to the calendar in the sidebar", async () => {
      await renderApp();
      expect(sidebar().getByRole("group", { name: "Calendar" })).toBeInTheDocument();
      for (const name of ["Day", "Week", "Month"]) expect(within(screen.getByRole("navigation", { name: "Views" })).queryByRole("button", { name })).not.toBeInTheDocument();
    });

    it("marks the active view in the sidebar", async () => {
      await renderApp({ view: "projects" });
      expect(sidebarEntry("Projects")).toHaveAttribute("aria-current", "page");
      expect(sidebarEntry("Hours")).not.toHaveAttribute("aria-current");
    });

    it("marks Hours while the Day view shows today", async () => {
      await renderApp();
      expect(sidebarEntry("Hours")).toHaveAttribute("aria-current", "page");
    });

    it("shows a toast for an error launch event", async () => {
      mocked.takeLaunchEvents.mockResolvedValueOnce([{ type: "error", message: "Could not open the link" }]);
      await renderApp();
      expect(await screen.findByRole("alert")).toHaveTextContent("Could not open the link");
    });

    it("shows an info toast for a notice launch event", async () => {
      mocked.takeLaunchEvents.mockResolvedValueOnce([{ type: "notice", message: "Opened the settings" }]);
      await renderApp();
      const message = await screen.findByText("Opened the settings");
      expect(message.closest('[role="status"]')).not.toBeNull();
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    });

    it("shows a toast when the launch events cannot be read", async () => {
      mocked.takeLaunchEvents.mockRejectedValueOnce({ kind: "io", message: "Channel closed", code: null });
      await renderApp();
      expect(await screen.findByRole("alert")).toHaveTextContent("Launch request failed: Channel closed");
    });
  });

  describe("welcome", () => {
    it("shows the welcome sheet on the first run", async () => {
      mocked.getUiState.mockResolvedValueOnce({ ...EMPTY_UI_STATE, onboardingDone: false });
      await renderApp();
      expect(await screen.findByRole("dialog", { name: "Welcome to TimeTracker" })).toBeInTheDocument();
    });

    it("does not show the welcome sheet once the onboarding is done", async () => {
      await renderApp();
      expect(screen.queryByRole("dialog", { name: "Welcome to TimeTracker" })).not.toBeInTheDocument();
    });

    it("holds back the permission dialog until the welcome is finished", async () => {
      mocked.getUiState.mockResolvedValueOnce({ ...EMPTY_UI_STATE, onboardingDone: false });
      await renderApp();
      await screen.findByRole("dialog", { name: "Welcome to TimeTracker" });
      act(() => mocked.onTrackerStatus.mock.calls[0][0](trackerStatus({ state: { kind: "needsPermission" }, permission: "denied", text: "Needs permission" })));
      expect(screen.queryByRole("dialog", { name: "Allow TimeTracker to read window titles" })).not.toBeInTheDocument();
      await userEvent.keyboard("{Escape}");
      expect(await screen.findByRole("dialog", { name: "Allow TimeTracker to read window titles" })).toBeInTheDocument();
    });
  });

  describe("backend events", () => {
    it("shows the day of a showDay launch event in the Day view", async () => {
      mocked.takeLaunchEvents.mockResolvedValueOnce([{ type: "showDay", day: "2026-10-20" }]);
      await renderApp({ view: "month" });
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenCalledWith("2026-10-20"));
      expect(mainHeading("Day")).toBeInTheDocument();
    });

    it("shows the day of a showDay request from another process and replies without an error", async () => {
      await renderApp({ view: "reports" });
      const onRemote = mocked.onRemote.mock.calls[0][0];
      act(() => void onRemote({ id: "3", request: { type: "showDay", day: "2026-10-19" } }));
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenCalledWith("2026-10-19"));
      await vi.waitFor(() => expect(mocked.remoteReply).toHaveBeenCalledWith("3", null, null));
    });

    it("reloads the shown day when its data changed", async () => {
      const day = await api.calendarStep("day", today(), -1);
      mocked.dayView.mockResolvedValue(dayView({ day, totals: { recordedMs: 0, trackedMs: 600_000, awayMs: 0 } }));
      await renderApp({ day });
      await screen.findByText("tracked 0:10");
      mocked.dayView.mockResolvedValue(dayView({ day, totals: { recordedMs: 0, trackedMs: 1_800_000, awayMs: 0 } }));
      act(() => mocked.onDataChanged.mock.calls[0][0]({ kind: "activity", day }));
      expect(await screen.findByText("tracked 0:30")).toBeInTheDocument();
    });

    it("opens on the day Rust sends as the one shown last", async () => {
      mocked.getUiState.mockResolvedValueOnce({ ...EMPTY_UI_STATE, lastDay: "2026-10-05" });
      await renderApp();
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenLastCalledWith("2026-10-05"));
    });

    it("opens on today when Rust sends no day shown last", async () => {
      await renderApp();
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenLastCalledWith(today()));
    });
  });

  describe("reports", () => {
    it("lists the saved reports under Reports on boot", async () => {
      mocked.savedReportsList.mockResolvedValueOnce([savedReport({ name: "Weekly" })]);
      await renderApp();
      expect(await within(screen.getByRole("group", { name: "Saved reports" })).findByRole("button", { name: "Weekly" })).toBeInTheDocument();
    });

    it("shows the Reports view", async () => {
      await renderApp({ view: "reports" });
      expect(await screen.findByRole("region", { name: "Reports" })).toBeInTheDocument();
    });
  });

  describe("projects", () => {
    const website = projectItem({ project: { id: "p-site", name: "Website" } });

    beforeEach(() => {
      mocked.projectsList.mockResolvedValue([]);
      mocked.tagsList.mockResolvedValue([]);
    });

    it("loads the projects on boot", async () => {
      mocked.projectsList.mockResolvedValue([website]);
      await renderApp({ view: "projects" });
      expect(await screen.findByRole("button", { name: "Website" })).toBeInTheDocument();
    });

    it("reloads the projects when projects changed", async () => {
      await renderApp({ view: "projects" });
      mocked.projectsList.mockResolvedValue([website]);
      act(() => mocked.onDataChanged.mock.calls[0][0]({ kind: "projects", day: null }));
      expect(await screen.findByRole("button", { name: "Website" })).toBeInTheDocument();
    });

    it("shows the new hours on the Projects page when recordings changed", async () => {
      mocked.projectsList.mockResolvedValue([projectItem({ project: { id: "p-site", name: "Website" }, weekMs: 3_600_000 })]);
      await renderApp({ view: "projects" });
      expect(await screen.findByTitle("This week: 1:00 h")).toBeInTheDocument();
      mocked.projectsList.mockResolvedValue([projectItem({ project: { id: "p-site", name: "Website" }, weekMs: 7_200_000 })]);
      act(() => mocked.onDataChanged.mock.calls[0][0]({ kind: "recordings", day: null }));
      expect(await screen.findByTitle("This week: 2:00 h")).toBeInTheDocument();
    });

    it("opens the new project sheet with ⌘N in the Projects view", async () => {
      await renderApp({ view: "projects" });
      await userEvent.keyboard("{Meta>}n{/Meta}");
      expect(await screen.findByRole("dialog", { name: "New project" })).toBeInTheDocument();
    });

    it("ignores ⌘N in other views", async () => {
      await renderApp({ view: "day" });
      await userEvent.keyboard("{Meta>}n{/Meta}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("finds a project through the palette and opens its report", async () => {
      mocked.projectsList.mockResolvedValueOnce([website]);
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search"), "p:web{Enter}");
      expect(await screen.findByRole("region", { name: "Reports" })).toBeInTheDocument();
    });
  });

  describe("tags", () => {
    it("lists the tags of the backend in the Tags view", async () => {
      mocked.tagsList.mockResolvedValue([tag({ name: "billing" })]);
      await renderApp({ view: "tags" });
      expect(await screen.findByText("billing")).toBeInTheDocument();
      expect(mainHeading("Tags")).toBeInTheDocument();
    });

    it("reloads the tags when tags changed", async () => {
      mocked.tagsList.mockResolvedValue([]);
      await renderApp({ view: "tags" });
      mocked.tagsList.mockResolvedValue([tag({ name: "billing" })]);
      act(() => mocked.onDataChanged.mock.calls[0][0]({ kind: "tags", day: null }));
      expect(await screen.findByText("billing")).toBeInTheDocument();
    });
  });

  describe("new tag", () => {
    it("opens the new tag prompt with ⌘N in the Tags view and saves the typed name", async () => {
      await renderApp({ view: "tags" });
      await userEvent.keyboard("{Meta>}n{/Meta}");
      const dialog = await screen.findByRole("dialog", { name: "New tag" });
      await userEvent.type(within(dialog).getByLabelText("Name"), "billing");
      await userEvent.click(within(dialog).getByRole("button", { name: "Add" }));
      await vi.waitFor(() => expect(mocked.tagsSave).toHaveBeenCalledWith(null, "billing"));
      expect(screen.queryByRole("dialog", { name: "New tag" })).not.toBeInTheDocument();
    });

    it("opens no new tag prompt with ⌘N in the Rules view", async () => {
      await renderApp({ view: "rules" });
      await userEvent.keyboard("{Meta>}n{/Meta}");
      expect(await screen.findByRole("dialog", { name: "New rule" })).toBeInTheDocument();
      expect(screen.queryByRole("dialog", { name: "New tag" })).not.toBeInTheDocument();
    });
  });

  describe("rules", () => {
    it("lists the rules of the backend in the Rules view", async () => {
      mocked.rulesList.mockResolvedValueOnce([rule({ name: "Chat" })]);
      await renderApp({ view: "rules" });
      expect(await within(screen.getByRole("list", { name: "Rules in order" })).findByText("Chat")).toBeInTheDocument();
    });

    it("reloads the rules when rules changed", async () => {
      await renderApp({ view: "rules" });
      await screen.findByText("No rules yet.");
      mocked.rulesList.mockResolvedValueOnce([rule({ name: "Chat" })]);
      act(() => mocked.onDataChanged.mock.calls[0][0]({ kind: "rules", day: null }));
      expect(await screen.findByText("Chat")).toBeInTheDocument();
    });

    it("opens the new rule sheet with ⌘N in the Rules view", async () => {
      await renderApp({ view: "rules" });
      await userEvent.keyboard("{Meta>}n{/Meta}");
      expect(await screen.findByRole("dialog", { name: "New rule" })).toBeInTheDocument();
    });

    it("opens the new project sheet, not a rule, with ⌘N in the Projects view", async () => {
      await renderApp({ view: "projects" });
      await userEvent.keyboard("{Meta>}n{/Meta}");
      expect(await screen.findByRole("dialog", { name: "New project" })).toBeInTheDocument();
      expect(screen.queryByRole("dialog", { name: "New rule" })).not.toBeInTheDocument();
    });
  });

  describe("navigation", () => {
    it("switches the view when a sidebar entry is clicked", async () => {
      await renderApp();
      await userEvent.click(sidebarEntry("Reports"));
      expect(mainHeading("Reports")).toBeInTheDocument();
    });

    it("shows the Day view of today from Hours", async () => {
      await renderApp({ view: "projects" });
      await userEvent.click(sidebarEntry("Hours"));
      expect(mainHeading("Day")).toBeInTheDocument();
      expect(useStore.getState().day).toBe(today());
    });

    it("shows the Day view of a day clicked in the sidebar's calendar", async () => {
      mocked.calendarMiniMonth.mockImplementation(async (shownDay, offset) => miniMonth({ monthStart: await api.calendarStep("month", shownDay, offset) }));
      await renderApp({ view: "projects" });
      await userEvent.click(await sidebar().findByRole("button", { name: today() }));
      expect(mainHeading("Day")).toBeInTheDocument();
      await vi.waitFor(() => expect(mocked.dayView).toHaveBeenLastCalledWith(today()));
    });

    it.each<[string, string, View]>([
      ["{Meta>}1{/Meta}", "Day", "projects"],
      ["{Meta>}2{/Meta}", "Week", "day"],
      ["{Meta>}3{/Meta}", "Month", "day"],
      ["{Meta>}4{/Meta}", "Reports", "day"],
      ["{Meta>}5{/Meta}", "Projects", "day"],
      ["{Meta>}6{/Meta}", "Tags", "day"],
      ["{Meta>}7{/Meta}", "Rules", "day"],
    ])("shows the view for %s", async (keys, heading, from) => {
      await renderApp({ view: from });
      await userEvent.keyboard(keys);
      expect(mainHeading(heading)).toBeInTheDocument();
    });
  });

  describe("sidebar", () => {
    it("hides with ⌘⌃S and returns with the same keys", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}{Control>}s{/Control}{/Meta}");
      expect(screen.queryByRole("complementary", { name: "Sidebar" })).not.toBeInTheDocument();
      await userEvent.keyboard("{Meta>}{Control>}s{/Control}{/Meta}");
      expect(screen.getByRole("complementary", { name: "Sidebar" })).toBeInTheDocument();
    });

    it("shows the tracker status in the sidebar, and in the status bar only while the sidebar is hidden", async () => {
      mocked.trackerStatus.mockResolvedValue(trackerStatus({ text: "Tracking · Mail" }));
      await renderApp();
      expect(await sidebar().findByText("Tracking · Mail")).toBeInTheDocument();
      expect(within(screen.getByRole("contentinfo")).queryByText("Tracking · Mail")).not.toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Toggle sidebar" }));
      expect(await within(screen.getByRole("contentinfo")).findByText("Tracking · Mail")).toBeInTheDocument();
    });

    it("hides when the status bar button is clicked", async () => {
      await renderApp();
      await userEvent.click(screen.getByRole("button", { name: "Toggle sidebar" }));
      expect(screen.queryByRole("complementary", { name: "Sidebar" })).not.toBeInTheDocument();
    });
  });

  describe("tracker", () => {
    it("shows the status loaded on boot in the sidebar", async () => {
      mocked.trackerStatus.mockResolvedValueOnce(trackerStatus({ text: "Tracking · Mail" }));
      await renderApp();
      expect(await sidebar().findByText("Tracking · Mail")).toBeInTheDocument();
    });

    it("shows that tracking is off when the status cannot be loaded", async () => {
      mocked.trackerStatus.mockRejectedValueOnce({ kind: "unavailable", message: "No data store", code: null });
      await renderApp();
      expect(await sidebar().findByText("Tracking off")).toBeInTheDocument();
    });

    it("follows the tracker status events", async () => {
      await renderApp();
      const onStatus = mocked.onTrackerStatus.mock.calls[0][0];
      act(() => onStatus(trackerStatus({ text: "Tracking · Safari", current: activityRow() })));
      expect(sidebar().getByText("Tracking · Safari")).toBeInTheDocument();
    });

    it("asks for the permission while the tracker needs it", async () => {
      await renderApp();
      const onStatus = mocked.onTrackerStatus.mock.calls[0][0];
      act(() => onStatus(trackerStatus({ state: { kind: "needsPermission" }, permission: "denied", text: "Needs permission" })));
      expect(await screen.findByRole("dialog", { name: "Allow TimeTracker to read window titles" })).toBeInTheDocument();
    });
  });

  describe("timer", () => {
    const statusBar = () => screen;

    it("shows the timer loaded on boot in the status bar", async () => {
      mocked.timerStatus.mockResolvedValueOnce(runningStatus({ projectName: "Docs" }));
      await renderApp();
      expect(await statusBar().findByRole("button", { name: "⏱ Docs 1:23" })).toBeInTheDocument();
    });

    it("follows the timer status events and stops the timer from the status bar", async () => {
      await renderApp();
      expect(statusBar().queryByTitle("Stop timer")).not.toBeInTheDocument();
      act(() => mocked.onTimerStatus.mock.calls[0][0](runningStatus({ projectName: "Docs" })));
      await userEvent.click(statusBar().getByRole("button", { name: "⏱ Docs 1:23" }));
      expect(mocked.timerStop).toHaveBeenCalled();
      act(() => mocked.onTimerStatus.mock.calls[0][0]({ running: null }));
      expect(statusBar().queryByTitle("Stop timer")).not.toBeInTheDocument();
    });
  });

  describe("permission dialog", () => {
    const needsPermission = () => trackerStatus({ state: { kind: "needsPermission" }, permission: "denied", text: "Needs permission" });
    const dialogName = "Allow TimeTracker to read window titles";

    it("reopens from the sidebar's Allow… button after it was dismissed", async () => {
      await renderApp();
      act(() => mocked.onTrackerStatus.mock.calls[0][0](needsPermission()));
      await screen.findByRole("dialog", { name: dialogName });
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog", { name: dialogName })).not.toBeInTheDocument();

      await userEvent.click(sidebar().getByRole("button", { name: "Allow…" }));
      expect(await screen.findByRole("dialog", { name: dialogName })).toBeInTheDocument();
    });

    it("shows again on its own when the permission is lost anew after a dismissal", async () => {
      await renderApp();
      const onStatus = mocked.onTrackerStatus.mock.calls[0][0];
      act(() => onStatus(needsPermission()));
      await screen.findByRole("dialog", { name: dialogName });
      await userEvent.keyboard("{Escape}");

      act(() => onStatus(trackerStatus()));
      act(() => onStatus(needsPermission()));
      expect(await screen.findByRole("dialog", { name: dialogName })).toBeInTheDocument();
    });
  });

  describe("settings", () => {
    it("opens with ⌘, and closes with it again", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>},{/Meta}");
      const dialog = await screen.findByRole("dialog");
      expect(within(dialog).getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
      await userEvent.keyboard("{Meta>},{/Meta}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("opens from the command palette", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search"), "settings");
      await userEvent.click(screen.getByRole("option", { name: /^Settings…/ }));
      expect(await screen.findByRole("heading", { name: "Appearance" })).toBeInTheDocument();
    });

    it("opens from the gear in the sidebar", async () => {
      await renderApp();
      await userEvent.click(sidebar().getByRole("button", { name: "Settings" }));
      expect(await screen.findByRole("heading", { name: "Appearance" })).toBeInTheDocument();
    });

    it("closes with Escape", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>},{/Meta}");
      await screen.findByRole("dialog");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });

  describe("command palette", () => {
    it("opens with ⌘K", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      expect(await screen.findByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
    });

    it("filters the commands by the typed text", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search"), "zoom");
      const options = screen.getAllByRole("option").map((o) => o.textContent);
      expect(options).toHaveLength(3);
      expect(options.join(" ")).toMatch(/Zoom in.*Zoom out.*Reset zoom/);
    });

    it("says so when no command matches", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search"), "qqq");
      expect(screen.getByText("No matches.")).toBeInTheDocument();
    });

    it("runs the chosen command on Enter and closes", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search"), ">show week{Enter}");
      expect(mainHeading("Week")).toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("switches to a view that is clicked", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.click(await screen.findByRole("option", { name: /^Projects/ }));
      expect(mainHeading("Projects")).toBeInTheDocument();
    });

    it("closes with Escape without running anything", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await screen.findByRole("dialog");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mainHeading("Day")).toBeInTheDocument();
    });
  });
});
