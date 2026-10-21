import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Sun } from "lucide-react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, type DateJump } from "../api";
import { installCommands } from "../commands";
import { registerSettingsPage } from "../registry/settingsPages";
import { resetStore, useStore, type Store } from "../state/store";
import { client, projectItem, projectsStore } from "../test/projectFixtures";
import { savedReport } from "../test/reportFixtures";
import { CommandPalette } from "./CommandPalette";
import { Dialogs } from "./Dialogs";

const mocked = vi.mocked(api, true);

registerSettingsPage({ id: "zebra", group: "Interface", label: "Zebra", icon: Sun, Body: () => null });

const acme = client({ id: "c-acme", name: "Acme" });
const oldClient = client({ id: "c-old", name: "Oldco", archivedAt: 5 });
const promo = projectItem({ project: { id: "p-promo", name: "Promo", clientId: acme.id }, client: acme });
const solo = projectItem({ project: { id: "p-solo", name: "Solo" } });
const archived = projectItem({ project: { id: "p-old", name: "Promo archive", archivedAt: 5 } });
const weekly = savedReport({ id: "s1", name: "Weekly" });

const today: DateJump = { name: "Today", unit: "day", from: "2026-03-10", to: "2026-03-10" };

async function openPalette(query = "", patch: Partial<Store> = {}) {
  resetStore({ ...projectsStore({ projects: [promo, solo, archived], clients: [acme, oldClient] }), paletteOpen: true, view: "projects", ...patch });
  installCommands();
  render(
    <>
      <CommandPalette />
      <Dialogs />
    </>,
  );
  const input = screen.getByLabelText("Search");
  if (query) await userEvent.type(input, query);
}

const options = () => screen.queryAllByRole("option").map((o) => o.textContent ?? "");
const indexOfOption = (pattern: RegExp) => options().findIndex((t) => pattern.test(t));
const choose = (query: string, patch: Partial<Store> = {}) => openPalette(query, patch).then(() => userEvent.keyboard("{Enter}"));

describe("CommandPalette", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.calendarJumps.mockResolvedValue([]);
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
      const reports = indexOfOption(/^Reports/);
      const copy = indexOfOption(/^Copy report as text/);
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

    it.each([
      ["Yesterday", "day", "day", "2026-03-09"],
      ["Last week", "week", "week", "2026-03-02"],
      ["March", "month", "month", "2026-03-01"],
    ] as const)("shows the day of the date %s in its own view", async (name, unit, view, from) => {
      mocked.calendarJumps.mockResolvedValue([{ name, unit, from, to: from }]);
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
      expect(useStore.getState().dialog).toMatchObject({ type: "settings", page: "zebra" });
      expect(useStore.getState().paletteOpen).toBe(false);
    });

    it("runs the command", async () => {
      await choose(">new project");
      expect(await screen.findByRole("dialog", { name: "New project" })).toBeInTheDocument();
      expect(useStore.getState().paletteOpen).toBe(false);
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

  describe("prefixes", () => {
    it("lists only commands after >, the views' commands included", async () => {
      await openPalette(">", { recent: [promo.project], savedReports: [weekly] });
      expect(indexOfOption(/^Show Week/i)).toBeGreaterThanOrEqual(0);
      expect(indexOfOption(/^New project/)).toBeGreaterThanOrEqual(0);
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
      mocked.calendarJumps.mockResolvedValue([{ name: "Monday", unit: "day", from: "2026-03-09", to: "2026-03-09" }]);
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
