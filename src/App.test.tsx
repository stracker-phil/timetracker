import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./api", async (orig) => ({ ...(await orig<object>()), api: (await import("./test/apiMock")).createApiMock() }));

import App from "./App";
import { api } from "./api";
import { resetStore, type View } from "./state/store";
import { activityRow, trackerStatus } from "./test/data";
import { projectItem, tag } from "./test/projectFixtures";

const mocked = vi.mocked(api, true);

/** Renders the app and waits until the settings arrived and the commands are installed. */
async function renderApp(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore(patch);
  render(<App />);
  await screen.findByRole("button", { name: "13 px" });
}

/** The Day view is the activity list (a region), every other view a placeholder with a level-1 heading. */
const mainHeading = (name: string) => (name === "Day" ? screen.getByRole("region", { name: "Activity" }) : screen.getByRole("heading", { level: 1, name }));
const sidebarEntry = (name: string) => within(screen.getByRole("navigation", { name: "Views" })).getByRole("button", { name: new RegExp(`^${name}`) });

describe("App", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("shell", () => {
    it("shows the toolbar title, the status bar and the Day view", async () => {
      await renderApp();
      expect(screen.getByText("TimeTracker")).toBeInTheDocument();
      expect(screen.getByText("No activity recorded yet")).toBeInTheDocument();
      expect(mainHeading("Day")).toBeInTheDocument();
    });

    it.each(["Day", "Week", "Month", "Reports", "Projects"])("lists %s in the sidebar", async (name) => {
      await renderApp();
      expect(sidebarEntry(name)).toBeInTheDocument();
    });

    it("marks the active view in the sidebar", async () => {
      await renderApp({ view: "month" });
      expect(sidebarEntry("Month")).toHaveAttribute("aria-current", "page");
      expect(sidebarEntry("Day")).not.toHaveAttribute("aria-current");
    });

    it("shows a toast for an error launch event", async () => {
      mocked.takeLaunchEvents.mockResolvedValueOnce([{ type: "error", message: "Could not open the link" }]);
      await renderApp();
      expect(await screen.findByRole("alert")).toHaveTextContent("Could not open the link");
    });

    it("shows a toast when the launch events cannot be read", async () => {
      mocked.takeLaunchEvents.mockRejectedValueOnce({ kind: "io", message: "Channel closed", code: null });
      await renderApp();
      expect(await screen.findByRole("alert")).toHaveTextContent("Launch request failed: Channel closed");
    });
  });

  describe("backend events", () => {
    afterEach(() => mocked.activityList.mockResolvedValue([]));

    it("shows the day of a showDay launch event in the Day view", async () => {
      mocked.takeLaunchEvents.mockResolvedValueOnce([{ type: "showDay", day: "2026-10-20" }]);
      await renderApp({ view: "month" });
      expect(await screen.findByLabelText("Day")).toHaveValue("2026-10-20");
      expect(mainHeading("Day")).toBeInTheDocument();
    });

    it("shows the day of a showDay request from another process and replies without an error", async () => {
      await renderApp({ view: "reports" });
      const onRemote = mocked.onRemote.mock.calls[0][0];
      act(() => void onRemote({ id: "3", request: { type: "showDay", day: "2026-10-19" } }));
      expect(await screen.findByLabelText("Day")).toHaveValue("2026-10-19");
      await vi.waitFor(() => expect(mocked.remoteReply).toHaveBeenCalledWith("3", null, null));
    });

    it("reloads the shown day when its data changed", async () => {
      mocked.activityList.mockResolvedValue([]);
      await renderApp();
      mocked.activityList.mockResolvedValue([activityRow({ app: "Planner", title: "Weekly sync" })]);
      const day = screen.getByLabelText<HTMLInputElement>("Day").value;
      const onChanged = mocked.onDataChanged.mock.calls[0][0];
      act(() => onChanged({ kind: "activity", day } as never));
      expect(await screen.findByRole("cell", { name: "Weekly sync" })).toBeInTheDocument();
    });
  });

  describe("projects", () => {
    const website = projectItem({ project: { id: "p-site", name: "Website" } });

    it("loads the projects on boot", async () => {
      mocked.projectsList.mockResolvedValueOnce([website]);
      await renderApp({ view: "projects" });
      expect(await screen.findByRole("button", { name: "Website" })).toBeInTheDocument();
    });

    it("reloads the projects when projects changed", async () => {
      await renderApp({ view: "projects" });
      mocked.projectsList.mockResolvedValueOnce([website]);
      act(() => mocked.onDataChanged.mock.calls[0][0]({ kind: "projects", day: null }));
      expect(await screen.findByRole("button", { name: "Website" })).toBeInTheDocument();
    });

    it("reloads the tags when tags changed", async () => {
      await renderApp({ view: "projects" });
      mocked.tagsList.mockResolvedValueOnce([tag({ name: "billing" })]);
      act(() => mocked.onDataChanged.mock.calls[0][0]({ kind: "tags", day: null }));
      expect(await screen.findByText("billing")).toBeInTheDocument();
    });

    it("leaves the projects alone when other data changed", async () => {
      await renderApp({ view: "projects" });
      mocked.projectsList.mockClear();
      act(() => mocked.onDataChanged.mock.calls[0][0]({ kind: "recordings", day: null }));
      await new Promise((resolve) => setTimeout(resolve, 20));
      expect(mocked.projectsList).not.toHaveBeenCalled();
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

    it("finds a project through the palette and opens its sheet", async () => {
      mocked.projectsList.mockResolvedValueOnce([website]);
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search commands"), "p:web{Enter}");
      expect(await screen.findByRole("dialog", { name: "Edit project" })).toBeInTheDocument();
    });
  });

  describe("navigation", () => {
    it("switches the view when a sidebar entry is clicked", async () => {
      await renderApp();
      await userEvent.click(sidebarEntry("Reports"));
      expect(mainHeading("Reports")).toBeInTheDocument();
    });

    it("switches the view from the toolbar", async () => {
      await renderApp();
      await userEvent.click(within(screen.getByRole("group", { name: "View" })).getByRole("button", { name: "Month" }));
      expect(mainHeading("Month")).toBeInTheDocument();
    });

    it.each<[string, string, View]>([
      ["{Meta>}1{/Meta}", "Day", "projects"],
      ["{Meta>}2{/Meta}", "Week", "day"],
      ["{Meta>}3{/Meta}", "Month", "day"],
      ["{Meta>}4{/Meta}", "Reports", "day"],
      ["{Meta>}5{/Meta}", "Projects", "day"],
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

    it("hides when the status bar button is clicked", async () => {
      await renderApp();
      await userEvent.click(screen.getByRole("button", { name: "Toggle sidebar" }));
      expect(screen.queryByRole("complementary", { name: "Sidebar" })).not.toBeInTheDocument();
    });
  });

  describe("tracker", () => {
    const sidebar = () => within(screen.getByRole("complementary", { name: "Sidebar" }));

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

  describe("permission dialog", () => {
    const needsPermission = () => trackerStatus({ state: { kind: "needsPermission" }, permission: "denied", text: "Needs permission" });
    const dialogName = "Allow TimeTracker to read window titles";

    it("reopens from the sidebar's Allow… button after it was dismissed", async () => {
      await renderApp();
      act(() => mocked.onTrackerStatus.mock.calls[0][0](needsPermission()));
      await screen.findByRole("dialog", { name: dialogName });
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog", { name: dialogName })).not.toBeInTheDocument();

      await userEvent.click(within(screen.getByRole("complementary", { name: "Sidebar" })).getByRole("button", { name: "Allow…" }));
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

    it("opens from the toolbar button", async () => {
      await renderApp();
      await userEvent.click(screen.getByRole("button", { name: "Settings" }));
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
      await userEvent.type(await screen.findByLabelText("Search commands"), "zoom");
      const options = screen.getAllByRole("option").map((o) => o.textContent);
      expect(options).toHaveLength(3);
      expect(options.join(" ")).toMatch(/Zoom in.*Zoom out.*Reset zoom/);
    });

    it("says so when no command matches", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search commands"), "qqq");
      expect(screen.getByText("No matches.")).toBeInTheDocument();
    });

    it("runs the chosen command on Enter and closes", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search commands"), "show week{Enter}");
      expect(mainHeading("Week")).toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("runs a command that is clicked", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.click(await screen.findByRole("option", { name: /Show Projects/ }));
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
