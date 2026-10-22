import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { installCommands } from "../commands";
import { today } from "../lib/day";
import { trackerStatus } from "../test/data";
import { defaultFilter } from "../state/slices/reports";
import { resetStore, useStore } from "../state/store";
import { savedReport } from "../test/reportFixtures";
import { settings } from "../test/settings";
import { CommandPalette } from "./CommandPalette";
import { Dialogs } from "./Dialogs";
import { Sidebar } from "./Sidebar";

const mocked = vi.mocked(api, true);

const weekly = savedReport({ id: "s1", name: "Weekly", filter: { ...defaultFilter(null), range: { kind: "lastWeek" }, groupBy: "tag" } });
const monthly = savedReport({ id: "s2", name: "Monthly", filter: { ...defaultFilter(null), range: { kind: "lastMonth" } } });

function renderSidebar(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore({ settings: settings(), savedReports: [weekly, monthly], ...patch });
  installCommands();
  render(
    <>
      <Sidebar />
      <CommandPalette />
      <Dialogs />
    </>,
  );
}

const saved = () => screen.getByRole("group", { name: "Saved reports" });

const openPalette = () => act(() => useStore.getState().setPaletteOpen(true, ">saved report"));
const savedReportCommand = (name: RegExp) => screen.queryByRole("option", { name });
const RENAME = /Rename saved report…/;
const DELETE = /Delete saved report…/;

describe("Sidebar rows", () => {
  beforeEach(() => vi.clearAllMocks());

  const views = () => within(screen.getByRole("navigation", { name: "Views" })).getAllByRole("button");
  const hours = () => within(screen.getByRole("navigation", { name: "Views" })).getByRole("button", { name: /^Hours/ });

  it("shows the calendar above the pages, and the tracker status below them", async () => {
    renderSidebar({ savedReports: [], tracker: trackerStatus({ text: "Tracking · Mail" }) });
    const calendar = await screen.findByRole("group", { name: "Calendar" });
    const nav = screen.getByRole("navigation", { name: "Views" });
    const status = screen.getByRole("status");
    expect(calendar.compareDocumentPosition(nav) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(nav.compareDocumentPosition(status) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(status).toHaveTextContent("Tracking · Mail");
  });

  it("lists Hours, Reports, Projects, Tags and Rules in this order", () => {
    renderSidebar({ savedReports: [] });
    ["Hours", "Reports", "Projects", "Tags", "Rules"].forEach((name, i) => expect(views()[i]).toHaveTextContent(new RegExp(`^${name}`)));
    expect(views()).toHaveLength(5);
  });

  it("separates Projects, Tags and Rules from Hours and Reports", () => {
    renderSidebar({ savedReports: [] });
    const separator = within(screen.getByRole("navigation", { name: "Views" })).getByRole("separator");
    expect(screen.getByRole("button", { name: /^Reports/ }).compareDocumentPosition(separator) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(separator.compareDocumentPosition(screen.getByRole("button", { name: /^Projects/ })) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("has no rows for Day, Week and Month, which the calendar shows", () => {
    renderSidebar({ savedReports: [] });
    for (const name of ["Day", "Week", "Month"]) expect(screen.queryByRole("button", { name: new RegExp(`^${name}`) })).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Date" })).not.toBeInTheDocument();
  });

  it("shows the key of a page next to it", () => {
    renderSidebar({ savedReports: [] });
    expect(screen.getByRole("button", { name: /^Tags/ })).toHaveTextContent("⌘6");
    expect(screen.getByRole("button", { name: /^Rules/ })).toHaveTextContent("⌘7");
  });

  it.each([
    ["Reports", "reports"],
    ["Projects", "projects"],
    ["Tags", "tags"],
    ["Rules", "rules"],
  ])("shows the %s view when its row is clicked", async (name, view) => {
    renderSidebar({ view: "day", savedReports: [] });
    await userEvent.click(screen.getByRole("button", { name: new RegExp(`^${name}`) }));
    expect(useStore.getState().view).toBe(view);
    expect(screen.getByRole("button", { name: new RegExp(`^${name}`) })).toHaveAttribute("aria-current", "page");
  });

  describe("Hours", () => {
    it("is selected while the Day view shows today", () => {
      renderSidebar({ view: "day", day: today(), savedReports: [] });
      expect(hours()).toHaveAttribute("aria-current", "page");
    });

    it.each([
      ["another day", { view: "day" as const, day: "2026-03-10" }],
      ["the Week view", { view: "week" as const, day: today() }],
      ["another view", { view: "projects" as const, day: today() }],
    ])("is not selected in %s", (_, patch) => {
      renderSidebar({ ...patch, savedReports: [] });
      expect(hours()).not.toHaveAttribute("aria-current");
    });

    it("shows today in the Day view", async () => {
      renderSidebar({ view: "reports", day: "2026-03-10", savedReports: [] });
      await userEvent.click(hours());
      expect(useStore.getState().view).toBe("day");
      expect(useStore.getState().day).toBe(today());
      expect(hours()).toHaveAttribute("aria-current", "page");
    });
  });
});

describe("Sidebar saved reports", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists the saved reports under Reports", () => {
    renderSidebar();
    expect(within(saved()).getAllByRole("button").map((b) => b.textContent)).toEqual(["Weekly", "Monthly"]);
  });

  it("shows no group when nothing is saved", () => {
    renderSidebar({ savedReports: [] });
    expect(screen.queryByRole("group", { name: "Saved reports" })).not.toBeInTheDocument();
  });

  it("opens the Reports view with the filter of the clicked report", async () => {
    renderSidebar({ view: "day" });
    await userEvent.click(within(saved()).getByRole("button", { name: "Weekly" }));
    expect(useStore.getState().view).toBe("reports");
    expect(useStore.getState().reportFilter).toEqual(weekly.filter);
  });

  describe("palette commands for the opened report", () => {
    it("offers no rename or delete before a saved report is opened", () => {
      renderSidebar({ view: "reports" });
      openPalette();
      expect(savedReportCommand(RENAME)).not.toBeInTheDocument();
      expect(savedReportCommand(DELETE)).not.toBeInTheDocument();
    });

    it("offers rename and delete after a saved report is opened from the sidebar", async () => {
      renderSidebar({ view: "day" });
      await userEvent.click(within(saved()).getByRole("button", { name: "Weekly" }));
      openPalette();
      expect(savedReportCommand(RENAME)).toBeInTheDocument();
      expect(savedReportCommand(DELETE)).toBeInTheDocument();
    });

    it("stops offering them once the filter changes", async () => {
      renderSidebar({ view: "day" });
      await userEvent.click(within(saved()).getByRole("button", { name: "Weekly" }));
      act(() => useStore.getState().setReportFilter({ groupBy: "project" }));
      openPalette();
      expect(savedReportCommand(RENAME)).not.toBeInTheDocument();
      expect(savedReportCommand(DELETE)).not.toBeInTheDocument();
    });

    it("renames the opened report through a prefilled prompt", async () => {
      mocked.savedReportRename.mockResolvedValue(savedReport({ id: "s1", name: "Last week" }));
      renderSidebar({ view: "day" });
      await userEvent.click(within(saved()).getByRole("button", { name: "Weekly" }));
      openPalette();
      await userEvent.click(screen.getByRole("option", { name: RENAME }));
      const input = await screen.findByRole("textbox", { name: "Name" });
      expect(input).toHaveValue("Weekly");
      await userEvent.clear(input);
      await userEvent.type(input, "Last week{Enter}");
      expect(mocked.savedReportRename).toHaveBeenCalledWith("s1", "Last week");
    });

    it("deletes the opened report after confirming", async () => {
      renderSidebar({ view: "day" });
      await userEvent.click(within(saved()).getByRole("button", { name: "Weekly" }));
      openPalette();
      await userEvent.click(screen.getByRole("option", { name: DELETE }));
      expect(await screen.findByText(/Delete the saved report "Weekly"/)).toBeInTheDocument();
      expect(mocked.savedReportDelete).not.toHaveBeenCalled();
      await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" }));
      expect(mocked.savedReportDelete).toHaveBeenCalledWith("s1");
    });
  });

  describe("context menu", () => {
    it("renames a report through a prefilled prompt", async () => {
      mocked.savedReportRename.mockResolvedValue(savedReport({ id: "s1", name: "Last week" }));
      renderSidebar();
      await userEvent.pointer({ keys: "[MouseRight]", target: within(saved()).getByRole("button", { name: "Weekly" }) });
      await userEvent.click(await screen.findByRole("menuitem", { name: "Rename…" }));
      const input = await screen.findByRole("textbox", { name: "Name" });
      expect(input).toHaveValue("Weekly");
      await userEvent.clear(input);
      await userEvent.type(input, "Last week{Enter}");
      expect(mocked.savedReportRename).toHaveBeenCalledWith("s1", "Last week");
      expect(await within(saved()).findByRole("button", { name: "Last week" })).toBeInTheDocument();
      expect(within(saved()).queryByRole("button", { name: "Weekly" })).not.toBeInTheDocument();
    });

    it("deletes a report after confirming", async () => {
      renderSidebar();
      await userEvent.pointer({ keys: "[MouseRight]", target: within(saved()).getByRole("button", { name: "Monthly" }) });
      await userEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
      expect(await screen.findByText(/Delete the saved report "Monthly"/)).toBeInTheDocument();
      expect(mocked.savedReportDelete).not.toHaveBeenCalled();
      await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" }));
      expect(mocked.savedReportDelete).toHaveBeenCalledWith("s2");
      await vi.waitFor(() => expect(within(saved()).queryByRole("button", { name: "Monthly" })).not.toBeInTheDocument());
      expect(within(saved()).getByRole("button", { name: "Weekly" })).toBeInTheDocument();
    });

    it("keeps the report when the delete is cancelled", async () => {
      renderSidebar();
      await userEvent.pointer({ keys: "[MouseRight]", target: within(saved()).getByRole("button", { name: "Monthly" }) });
      await userEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
      await userEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Cancel" }));
      expect(mocked.savedReportDelete).not.toHaveBeenCalled();
      expect(within(saved()).getByRole("button", { name: "Monthly" })).toBeInTheDocument();
    });
  });
});
