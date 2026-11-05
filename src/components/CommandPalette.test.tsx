import { act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Sun } from "lucide-react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, type DateJump, type Project, type SavedReport, type TrackerStatus } from "../api";
import { installCommands, runCommand } from "../commands";
import { registerSettingsPage } from "../registry/settingsPages";
import { resetStore, useStore, type Store } from "../state/store";
import { trackerStatus } from "../test/data";
import { menuCommand, menuCommands } from "../test/menuFixtures";
import { client, projectItem } from "../test/projectFixtures";
import { renderWithClient, SAVED_REPORTS_KEY, seedProjects, seedQuery, TRACKER_KEY } from "../test/queries";
import { savedReport } from "../test/reportFixtures";
import { CommandPalette } from "./CommandPalette";
import { Dialogs } from "./Dialogs";
import { TrackerLine } from "./TrackerStatus";

const mocked = vi.mocked(api, true);

registerSettingsPage({ id: "zebra", group: "Interface", label: "Zebra", icon: Sun, Body: () => null });

const acme = client({ id: "c-acme", name: "Acme" });
const oldClient = client({ id: "c-old", name: "Oldco", archivedAt: 5 });
const promo = projectItem({ project: { id: "p-promo", name: "Promo", clientId: acme.id }, client: acme });
const solo = projectItem({ project: { id: "p-solo", name: "Solo" } });
const archived = projectItem({ project: { id: "p-old", name: "Promo archive", archivedAt: 5 } });
const weekly = savedReport({ id: "s1", name: "Weekly" });

const today: DateJump = { name: "Today", unit: "day", from: "2026-03-10", to: "2026-03-10", label: "Tue, Mar 10" };

/** Store state, plus the query data the palette lists. */
type PaletteSetup = Partial<Store> & { recent?: Project[]; savedReports?: SavedReport[]; tracker?: TrackerStatus };

/** Opens the palette the way ⌘K does, which notes what the page can run before the sheet covers it. */
async function openPalette(query = "", { recent = [], savedReports = [], tracker, ...patch }: PaletteSetup = {}) {
  resetStore({ paletteOpen: false, view: "projects", ...patch });
  seedProjects({ projects: [promo, solo, archived], clients: [acme, oldClient], recent });
  seedQuery(SAVED_REPORTS_KEY, savedReports);
  if (tracker) seedQuery(TRACKER_KEY, tracker);
  installCommands();
  renderWithClient(
    <>
      <CommandPalette />
      <Dialogs />
    </>,
  );
  act(() => void runCommand("palette.toggle"));
  const input = await screen.findByLabelText("Search");
  if (query) await userEvent.type(input, query);
}

const options = () => screen.queryAllByRole("option").map((o) => o.textContent ?? "");
/** Waits until Rust's menu list reached the palette, for the tests that expect an item to be absent. */
async function menusLoaded() {
  await vi.waitFor(() => expect(mocked.menuCommands).toHaveBeenCalled());
  await act(async () => {});
}
const indexOfOption =(pattern: RegExp) => options().findIndex((t) => pattern.test(t));
const choose = (query: string, patch: PaletteSetup = {}) => openPalette(query, patch).then(() => userEvent.keyboard("{Enter}"));

describe("CommandPalette", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.calendarJumps.mockResolvedValue([]);
    mocked.menuCommands.mockResolvedValue(menuCommands());
  });

  describe("with nothing typed", () => {
    it("lists the views, the dates, the recent projects and the saved reports, but no commands", async () => {
      mocked.calendarJumps.mockResolvedValue([today]);
      await openPalette("", { recent: [promo.project, archived.project], savedReports: [weekly] });
      await screen.findByRole("option", { name: /^Today/ });
      expect(options()).toEqual([
        expect.stringMatching(/^Day/),
        expect.stringMatching(/^Week/),
        expect.stringMatching(/^Month/),
        expect.stringMatching(/^Reports/),
        expect.stringMatching(/^Projects/),
        expect.stringMatching(/^Tags/),
        expect.stringMatching(/^Rules/),
        expect.stringMatching(/^Today/),
        expect.stringContaining("Acme / Promo"),
        expect.stringMatching(/^Weekly/),
      ]);
      ["Go to", "Recent projects", "Saved reports"].forEach((name) => expect(screen.getByText(name)).toBeInTheDocument());
      expect(screen.queryByText("Commands")).not.toBeInTheDocument();
    });

    it("asks Rust for the default dates", async () => {
      await openPalette();
      await screen.findAllByRole("option");
      expect(mocked.calendarJumps).toHaveBeenCalledWith("");
    });

    it("shows no recent projects when none was used", async () => {
      await openPalette();
      expect(screen.queryByText("Recent projects")).not.toBeInTheDocument();
    });

    it("hints at the prefixes in the footer until something is typed", async () => {
      await openPalette();
      expect(screen.getByText("> commands · p: projects · c: clients · @ dates")).toBeInTheDocument();
      await userEvent.type(screen.getByLabelText("Search"), "d");
      expect(screen.queryByText(/> commands/)).not.toBeInTheDocument();
    });
  });

  describe("ranking", () => {
    it("lists the sections by their best match", async () => {
      await openPalette("rep", { view: "reports", savedReports: [savedReport({ id: "s2", name: "Prepare" })] });
      await screen.findByRole("option", { name: /^Copy Report as Text/ });
      const reports = indexOfOption(/^Reports/);
      const copy = indexOfOption(/^Copy Report as Text/);
      const prepare = indexOfOption(/^Prepare/);
      expect(reports).toBeGreaterThanOrEqual(0);
      expect(copy).toBeGreaterThan(reports);
      expect(prepare).toBeGreaterThan(copy);
    });

    it("lists the matches of a section best first", async () => {
      const saved = ["Prepare", "Report card", "Rep"].map((name, i) => savedReport({ id: `s${i}`, name }));
      await openPalette("rep", { savedReports: saved });
      const [exact, prefix, inside] = [/^Rep(?![a-z])/,/^Report card/, /^Prepare/].map(indexOfOption);
      expect(exact).toBeGreaterThanOrEqual(0);
      expect(prefix).toBeGreaterThan(exact);
      expect(inside).toBeGreaterThan(prefix);
    });

    it("matches a project name by letters that occur in order", async () => {
      await openPalette("p:pm");
      expect(options()).toEqual([expect.stringContaining("Acme / Promo")]);
    });
  });

  describe("choosing with Enter", () => {
    it("switches to the view", async () => {
      await choose("week", { view: "day" });
      expect(useStore.getState().view).toBe("week");
      expect(useStore.getState().paletteOpen).toBe(false);
    });

    it("lists the Day view once and has no Hours entry", async () => {
      await openPalette("day", { view: "projects", day: "2026-03-10" });
      expect(options().filter((o) => /^Day/.test(o))).toHaveLength(1);
      expect(screen.queryByRole("option", { name: /^Hours/ })).not.toBeInTheDocument();
    });

    it.each([
      ["Yesterday", "day", "day", "2026-03-09", "Mon, Mar 9"],
      ["Last week", "week", "week", "2026-03-02", "Mar 2 – 8"],
      ["March", "month", "month", "2026-03-01", "March 2026"],
    ] as const)("shows the day of the date %s in its own view", async (name, unit, view, from, label) => {
      mocked.calendarJumps.mockResolvedValue([{ name, unit, from, to: from, label }]);
      await openPalette(name.toLowerCase(), { view: "projects" });
      await screen.findByRole("option", { name: new RegExp(`^${name}`) });
      await userEvent.keyboard("{Enter}");
      expect(useStore.getState().view).toBe(view);
      expect(useStore.getState().day).toBe(from);
      expect(useStore.getState().paletteOpen).toBe(false);
    });

    it("opens the Reports view filtered to the project and closes", async () => {
      await choose("p:promo", { view: "day" });
      const state = useStore.getState();
      expect(state.view).toBe("reports");
      expect(state).toMatchObject({ reportFilter: { projectIds: ["p-promo"], clientIds: [] } });
      expect(state.paletteOpen).toBe(false);
    });

    it("opens the Reports view filtered to the client", async () => {
      await choose("c:acme", { view: "day" });
      const state = useStore.getState();
      expect(state.view).toBe("reports");
      expect(state).toMatchObject({ reportFilter: { clientIds: ["c-acme"], projectIds: [] } });
    });

    it("opens the saved report", async () => {
      await choose("weekly", { view: "day", savedReports: [weekly] });
      const state = useStore.getState();
      expect(state.view).toBe("reports");
      expect(state.reportFilter).toEqual(weekly.filter);
      expect(state.openedReport).toBe("s1");
    });

    it("opens Settings on the chosen page", async () => {
      await choose("zebra");
      expect(mocked.openSettings).toHaveBeenCalledWith("zebra");
      expect(useStore.getState().paletteOpen).toBe(false);
    });

    it("runs a page command of the menus directly", async () => {
      await openPalette(">new project");
      await screen.findByRole("option", { name: /^New Project…/ });
      await userEvent.keyboard("{Enter}");
      expect(await screen.findByRole("dialog", { name: "New project" })).toBeInTheDocument();
      expect(useStore.getState().paletteOpen).toBe(false);
      expect(mocked.runCommand).not.toHaveBeenCalled();
    });

    it("asks the bus to run an app item of the menus and closes", async () => {
      await openPalette(">start timer");
      await screen.findByRole("option", { name: /^Start Timer…/ });
      await userEvent.keyboard("{Enter}");
      expect(mocked.runCommand).toHaveBeenCalledWith("timer:start");
      expect(useStore.getState().paletteOpen).toBe(false);
    });

    it("shows a toast when the bus refuses an app item", async () => {
      mocked.runCommand.mockRejectedValueOnce({ kind: "invalid_input", message: "No timer runs", code: null });
      await openPalette(">start timer");
      await screen.findByRole("option", { name: /^Start Timer…/ });
      await userEvent.keyboard("{Enter}");
      await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message)).toEqual(["Couldn't run Start Timer…: No timer runs"]));
    });

    it("chooses the row that is clicked", async () => {
      await openPalette("", { view: "day" });
      await userEvent.click(screen.getByRole("option", { name: /^Projects/ }));
      expect(useStore.getState().view).toBe("projects");
    });

    it("moves the selection with the arrow keys", async () => {
      await openPalette("", { view: "projects" });
      await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowUp}{Enter}");
      expect(useStore.getState().view).toBe("week");
    });
  });

  describe("starting a timer with ⌘Enter", () => {
    it("starts a timer on the project and names the key in the footer", async () => {
      await openPalette("p:promo");
      expect(screen.getByText(/⌘↵ start timer/)).toBeInTheDocument();
      await userEvent.keyboard("{Meta>}{Enter}{/Meta}");
      expect(mocked.timerStart).toHaveBeenCalledWith("p-promo", "", null);
      expect(useStore.getState().paletteOpen).toBe(false);
    });

    it("names no ⌘Enter action for a view", async () => {
      await openPalette("week");
      expect(screen.queryByText(/⌘↵/)).not.toBeInTheDocument();
    });
  });

  describe("menu commands", () => {
    it("lists the items of the menus with their keys, in menu order, under the menus' titles", async () => {
      await openPalette(">");
      await screen.findByRole("option", { name: /^Settings…/ });
      expect(options()).toEqual(["Settings…⌘,", "Start Timer…⌘T", "New Project…⌘N", "Day⌘1", "Week⌘2", "Zoom In⌘=", "Zoom Out⌘-", "Reset Zoom⌘0"]);
      ["TimeTracker", "File", "View"].forEach((menu) => expect(screen.getByText(menu)).toBeInTheDocument());
    });

    it("matches an item by its label", async () => {
      await openPalette("zoom");
      await screen.findByRole("option", { name: /^Zoom In/ });
      expect(options()).toEqual(["Zoom In⌘=", "Zoom Out⌘-", "Reset Zoom⌘0"]);
    });

    it("shows nothing of the menus while nothing is typed", async () => {
      await openPalette();
      await menusLoaded();
      expect(indexOfOption(/^Start Timer…/)).toBe(-1);
      expect(screen.queryByText("File")).not.toBeInTheDocument();
    });

    it("leaves a page item out when its command could not run as the palette opened", async () => {
      await openPalette(">new", { view: "projects" });
      await screen.findByRole("option", { name: /^New Project…/ });
      expect(indexOfOption(/^New Rule…/)).toBe(-1);
    });

    it("lists a page item once its command can run", async () => {
      await openPalette(">new", { view: "rules" });
      await screen.findByRole("option", { name: /^New Rule…/ });
      expect(indexOfOption(/^New Project…/)).toBe(-1);
    });

    it("lists app items whatever the page shows", async () => {
      await openPalette(">timer", { view: "rules" });
      expect(await screen.findByRole("option", { name: /^Start Timer…/ })).toBeInTheDocument();
    });

    it("leaves the views to Go to without >", async () => {
      await openPalette("week");
      await menusLoaded();
      expect(options().filter((o) => /^Week/.test(o))).toHaveLength(1);
      expect(screen.queryByText("View")).not.toBeInTheDocument();
    });

    it("lists the views under the View menu after >", async () => {
      await openPalette(">week");
      expect(await screen.findByRole("option", { name: /^Week/ })).toBeInTheDocument();
      expect(screen.getByText("View")).toBeInTheDocument();
      expect(screen.queryByText("Go to")).not.toBeInTheDocument();
    });

    it("leaves out a page item whose command the page does not have", async () => {
      mocked.menuCommands.mockResolvedValue([menuCommand({ menu: "File", label: "Ghost", command: "page:ghost.command" })]);
      await openPalette(">ghost");
      await menusLoaded();
      expect(screen.getByText("No matches.")).toBeInTheDocument();
    });

    it("shows a toast when Rust cannot list the commands", async () => {
      mocked.menuCommands.mockRejectedValue({ kind: "io", message: "No menus", code: null });
      await openPalette(">");
      await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message)).toEqual(["Couldn't list the commands: No menus"]));
    });
  });

  describe("menus on screen", () => {
    const running = trackerStatus();
    const paused = trackerStatus({ state: { kind: "paused", until: null }, text: "Paused" });

    /** The tracker line offers its Pause menu while the tracker runs. */
    function mountTrackerLine() {
      seedQuery(TRACKER_KEY, running);
      return renderWithClient(<TrackerLine />);
    }

    it("lists the rows of a mounted menu under its section, and runs the chosen one", async () => {
      mountTrackerLine();
      await openPalette("pause", { tracker: running });
      const row = await screen.findByRole("option", { name: /^Pause tracking: For 1 hour/ });
      expect(within(screen.getByRole("listbox", { name: "Results" })).getByText("Tracking")).toBeInTheDocument();
      await userEvent.click(row);
      expect(mocked.trackerPause).toHaveBeenCalledWith({ minutes: 60 });
      expect(useStore.getState().paletteOpen).toBe(false);
    });

    it("lists no rows once the menu unmounted", async () => {
      mountTrackerLine().unmount();
      await openPalette("pause", { tracker: running });
      await menusLoaded();
      expect(indexOfOption(/^Pause tracking:/)).toBe(-1);
    });

    it("lists no rows while the tracker is paused", async () => {
      mountTrackerLine();
      act(() => seedQuery(TRACKER_KEY, paused));
      await vi.waitFor(() => expect(screen.getAllByText(/Paused/).length).toBeGreaterThan(0));
      await openPalette("pause", { tracker: paused });
      await menusLoaded();
      expect(indexOfOption(/^Pause tracking:/)).toBe(-1);
    });
  });

  describe("prefixes", () => {
    it("lists only commands after >, the views' commands included", async () => {
      await openPalette(">", { recent: [promo.project], savedReports: [weekly] });
      await screen.findByRole("option", { name: /^Week/ });
      expect(indexOfOption(/^Hours/)).toBe(-1);
      expect(indexOfOption(/^New Project…/)).toBeGreaterThanOrEqual(0);
      ["Go to", "Recent projects", "Projects", "Saved reports"].forEach((name) => expect(screen.queryByText(name)).not.toBeInTheDocument());
    });

    it("lists only the active projects after p:", async () => {
      await openPalette("p:", { savedReports: [weekly] });
      expect(options()).toEqual([expect.stringContaining("Acme / Promo"), expect.stringContaining("Solo")]);
    });

    it("lists only the active clients after c:", async () => {
      await openPalette("c:");
      expect(options()).toEqual(["Acme"]);
    });

    it("lists only dates after @ and looks up the text behind it", async () => {
      mocked.calendarJumps.mockResolvedValue([{ name: "Monday", unit: "day", from: "2026-03-09", to: "2026-03-09", label: "Mon, Mar 9" }]);
      await openPalette("@ mon", { savedReports: [savedReport({ name: "Monthly" })] });
      await screen.findByRole("option", { name: /^Monday/ });
      expect(options()).toHaveLength(1);
      expect(mocked.calendarJumps).toHaveBeenLastCalledWith("mon");
    });

    it("looks up the whole text when no prefix is typed", async () => {
      await openPalette("last week");
      await vi.waitFor(() => expect(mocked.calendarJumps).toHaveBeenLastCalledWith("last week"));
    });

    it("looks up no dates after other prefixes", async () => {
      await openPalette("p:");
      mocked.calendarJumps.mockClear();
      await userEvent.type(screen.getByLabelText("Search"), "promo");
      expect(mocked.calendarJumps).not.toHaveBeenCalled();
    });
  });

  describe("without results", () => {
    it.each([["p:archive"], ["c:oldco"], ["qqq"]])("says so for %s, hiding archived projects and clients", async (query) => {
      await openPalette(query);
      expect(screen.queryByRole("option")).not.toBeInTheDocument();
      expect(screen.getByText("No matches.")).toBeInTheDocument();
    });

    it("keeps archived projects out of the recent ones", async () => {
      await openPalette("", { recent: [archived.project] });
      expect(screen.queryByText("Recent projects")).not.toBeInTheDocument();
    });

    it("closes with Escape", async () => {
      await openPalette();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(useStore.getState().paletteOpen).toBe(false);
    });
  });
});
