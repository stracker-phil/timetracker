import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { installCommands } from "../commands";
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

describe("Sidebar date navigator", () => {
  beforeEach(() => vi.clearAllMocks());

  const views = () => within(screen.getByRole("navigation", { name: "Views" })).getAllByRole("button");

  it("shows the date next to the list of views", async () => {
    renderSidebar({ view: "day", savedReports: [] });
    expect(within(await screen.findByRole("group", { name: "Date" })).getByRole("button", { name: "Previous day" })).toBeInTheDocument();
    ["Day", "Week", "Month", "Reports", "Projects", "Rules"].forEach((name, i) => expect(views()[i]).toHaveTextContent(new RegExp(`^${name}`)));
    expect(views()).toHaveLength(6);
  });

  it.each(["projects", "rules"] as const)("keeps the views and shows no date in the %s view", (view) => {
    renderSidebar({ view });
    expect(screen.queryByRole("group", { name: "Date" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Previous/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Day/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Projects/ })).toBeInTheDocument();
  });

  it("steps the shown day with the date buttons", async () => {
    renderSidebar({ view: "day", day: "2026-03-10" });
    await userEvent.click(await screen.findByRole("button", { name: "Next day" }));
    expect(useStore.getState().day).toBe("2026-03-11");
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
