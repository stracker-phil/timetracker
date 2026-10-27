import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Range, type ReportFilter, type Rounding } from "../../api";
import { installCommands, runCommand } from "../../commands";
import { defaultFilter } from "../../state/slices/reports";
import { resetStore, useStore } from "../../state/store";
import { DAY } from "../../test/dayFixtures";
import { projectItem, projectsStore, tag } from "../../test/projectFixtures";
import { reportClient, reportResult, savedReport } from "../../test/reportFixtures";
import { settings } from "../../test/settings";
import { Dialogs } from "../Dialogs";
import { Toasts } from "../Toasts";
import { ReportsView } from "./ReportsView";

const mocked = vi.mocked(api, true);

const lastFilter = (): ReportFilter => mocked.reportRun.mock.lastCall![0];

async function renderView(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore({ settings: settings(), view: "reports", ...patch });
  installCommands();
  render(
    <>
      <ReportsView />
      <Dialogs />
      <Toasts />
    </>,
  );
  await screen.findByLabelText("Totals");
}

describe("ReportsView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.reportRun.mockResolvedValue(reportResult());
  });

  describe("result", () => {
    it("titles the report with the title Rust sends", async () => {
      mocked.reportRun.mockResolvedValue(reportResult({ from: "2026-03-09", to: "2026-03-15", title: "Mar 9 – 15" }));
      await renderView();
      expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Mar 9 – 15");
    });

    it("shows the totals, the charts and the table of the report", async () => {
      await renderView();
      expect(screen.getByLabelText("Totals")).toHaveTextContent("45m");
      expect(screen.getByRole("region", { name: "By client" })).toBeInTheDocument();
      expect(screen.getByRole("region", { name: "By tag" })).toBeInTheDocument();
      expect(screen.getByRole("region", { name: "Recordings by project" })).toBeInTheDocument();
    });

    it.each([
      ["a single day", "2026-03-09", "2026-03-09", false],
      ["several days", "2026-03-09", "2026-03-15", true],
    ])("shows the day of the recordings only for %s", async (_label, from, to, shown) => {
      mocked.reportRun.mockResolvedValue(reportResult({ from, to }));
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: "Expand all" }));
      const days = within(screen.getByRole("region", { name: "Recordings by project" })).queryAllByText("Wed, Oct 21");
      expect(days.length > 0).toBe(shown);
    });

    it("says there are no recordings and shows neither charts nor table when the report is empty", async () => {
      mocked.reportRun.mockResolvedValue(reportResult({ clients: [], totalMs: 0, totalRoundedMs: 0, recordingCount: 0, projectCount: 0 }));
      await renderView();
      expect(await screen.findByText("No recordings.")).toBeInTheDocument();
      expect(screen.queryByRole("region", { name: "By client" })).not.toBeInTheDocument();
      expect(screen.queryByRole("region", { name: "Recordings by project" })).not.toBeInTheDocument();
    });

    it("shows the error when the report fails", async () => {
      mocked.reportRun.mockRejectedValue({ kind: "io", message: "Database is locked", code: null });
      resetStore({ settings: settings(), view: "reports" });
      render(<ReportsView />);
      expect(await screen.findByText("Database is locked")).toBeInTheDocument();
    });
  });

  describe("filters", () => {
    it("runs today's report grouped and rounded as the settings say until the filter is changed", async () => {
      resetStore({ settings: settings({ reports: { rounding: "nearest15", defaultGroup: "week" } }), view: "reports" });
      render(<ReportsView />);
      await screen.findByLabelText("Totals");
      expect(lastFilter()).toEqual({ ...defaultFilter(null), range: { kind: "today" }, groupBy: "week", rounding: "nearest15" });
      await userEvent.click(screen.getByRole("button", { name: "Group copied text by" }));
      expect(screen.getByRole("menuitemcheckbox", { name: "By week" })).toHaveAttribute("aria-checked", "true");
    });

    it.each<[string, Range["kind"]]>([
      ["Last week", "lastWeek"],
      ["This month", "thisMonth"],
      ["Yesterday", "yesterday"],
    ])("runs the report again with the period %s", async (label, kind) => {
      await renderView();
      const period = screen.getByRole("combobox", { name: "Period" });
      await userEvent.selectOptions(period, label);
      await vi.waitFor(() => expect(lastFilter().range).toEqual({ kind }));
      expect(period).toHaveValue(kind);
    });

    it("no longer offers the rounding or archived projects in the filter bar", async () => {
      await renderView();
      expect(screen.queryByRole("combobox", { name: "Rounding" })).not.toBeInTheDocument();
      expect(screen.queryByRole("switch", { name: "Include archived" })).not.toBeInTheDocument();
    });

    it("runs the report again with the grouping chosen for the copied text, without copying", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: "Group copied text by" }));
      await userEvent.click(screen.getByRole("menuitemcheckbox", { name: "By tag" }));
      await vi.waitFor(() => expect(lastFilter().groupBy).toBe("tag"));
      expect(mocked.reportText).not.toHaveBeenCalled();
      await userEvent.click(screen.getByRole("button", { name: "Group copied text by" }));
      expect(screen.getByRole("menuitemcheckbox", { name: "By tag" })).toHaveAttribute("aria-checked", "true");
      expect(screen.getByRole("menuitemcheckbox", { name: "By project" })).toHaveAttribute("aria-checked", "false");
    });

    it.each<[string, Rounding]>([
      ["Nearest 15 min", "nearest15"],
      ["Up to 30 min", "up30"],
      ["No rounding", "none"],
    ])("runs the report again with the %s rounding", async (label, value) => {
      await renderView({ reportFilter: { ...defaultFilter(settings()), rounding: value === "none" ? "up5" : "none" } });
      await userEvent.click(screen.getByRole("button", { name: "Report options" }));
      const options = within(await screen.findByRole("dialog", { name: "Report options" }));
      await userEvent.selectOptions(options.getByRole("combobox", { name: "Rounding" }), label);
      await vi.waitFor(() => expect(lastFilter().rounding).toBe(value));
    });

    it("runs the report again with archived projects included from the report options", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: "Report options" }));
      const options = within(await screen.findByRole("dialog", { name: "Report options" }));
      await userEvent.click(options.getByRole("switch", { name: "Include archived" }));
      await vi.waitFor(() => expect(lastFilter().includeArchived).toBe(true));
    });

    it("shows the current rounding and archived setting in the report options", async () => {
      await renderView({ reportFilter: { ...defaultFilter(settings()), rounding: "up30", includeArchived: true } });
      await userEvent.click(screen.getByRole("button", { name: "Report options" }));
      const options = within(await screen.findByRole("dialog", { name: "Report options" }));
      expect(options.getByRole("combobox", { name: "Rounding" })).toHaveValue("up30");
      expect(options.getByRole("switch", { name: "Include archived" })).toBeChecked();
    });

    it("starts a custom range from the days shown", async () => {
      mocked.reportRun.mockResolvedValue(reportResult({ from: "2026-03-09", to: "2026-03-15" }));
      await renderView();
      await userEvent.selectOptions(screen.getByRole("combobox", { name: "Period" }), "Custom");
      expect(await screen.findByLabelText("From")).toHaveValue("2026-03-09");
      expect(screen.getByLabelText("To")).toHaveValue("2026-03-15");
      await vi.waitFor(() => expect(lastFilter().range).toEqual({ kind: "custom", from: "2026-03-09", to: "2026-03-15" }));
    });

    it("runs the report again with the custom dates edited", async () => {
      await renderView({ reportFilter: { ...defaultFilter(settings()), range: { kind: "custom", from: "2026-03-09", to: "2026-03-15" } } });
      fireEvent.change(screen.getByLabelText("From"), { target: { value: "2026-03-10" } });
      fireEvent.change(screen.getByLabelText("To"), { target: { value: "2026-03-12" } });
      await vi.waitFor(() => expect(lastFilter().range).toEqual({ kind: "custom", from: "2026-03-10", to: "2026-03-12" }));
    });

    it("hides the date inputs for a preset range", async () => {
      await renderView();
      expect(screen.queryByLabelText("From")).not.toBeInTheDocument();
    });

    it("narrows the report to the projects and tags picked", async () => {
      await renderView(projectsStore({ projects: [projectItem({ project: { id: "p-site", name: "Website" } })], tags: [tag({ name: "support" })] }));
      await userEvent.type(screen.getByRole("combobox", { name: "Projects" }), "Web{Enter}");
      await userEvent.type(screen.getByRole("combobox", { name: "Tags" }), "supp{Enter}");
      await vi.waitFor(() => expect(lastFilter()).toMatchObject({ projectIds: ["p-site"], tagIds: ["tag-support"] }));
    });
  });

  describe("copy as text", () => {
    it("copies the text of the report for the shown filter and confirms", async () => {
      mocked.reportText.mockResolvedValue("Tue Mar 10\n\nWebsite 0:45 h\n");
      await renderView({ reportFilter: { ...defaultFilter(settings()), range: { kind: "lastWeek" } } });
      await userEvent.click(screen.getByRole("button", { name: /Copy as text/ }));
      expect(await screen.findByText("Copied the report")).toBeInTheDocument();
      expect(mocked.reportText).toHaveBeenCalledWith(expect.objectContaining({ range: { kind: "lastWeek" } }));
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("Tue Mar 10\n\nWebsite 0:45 h\n");
    });

    it("offers the groupings in the chevron menu of the Copy as text group", async () => {
      await renderView();
      const group = within(screen.getByRole("group", { name: "Copy as text" }));
      const chevron = group.getByRole("button", { name: "Group copied text by" });
      expect(chevron).toHaveAttribute("aria-haspopup", "menu");
      await userEvent.click(chevron);
      expect(screen.getAllByRole("menuitemcheckbox").map((i) => i.textContent)).toEqual(["By project", "By tag", "By day", "By week", "By month"]);
      expect(screen.getByRole("menuitemcheckbox", { name: "By project" })).toHaveAttribute("aria-checked", "true");
    });

    it("copies the text grouped as currently chosen", async () => {
      await renderView({ reportFilter: { ...defaultFilter(settings()), groupBy: "month" } });
      await userEvent.click(within(screen.getByRole("group", { name: "Copy as text" })).getByRole("button", { name: /^Copy as text/ }));
      await vi.waitFor(() => expect(mocked.reportText).toHaveBeenCalledWith(expect.objectContaining({ groupBy: "month" })));
    });

    it("copies through the reports.copyText command", async () => {
      await renderView();
      act(() => void runCommand("reports.copyText"));
      expect(await screen.findByText("Copied the report")).toBeInTheDocument();
    });

    it("opens the save dialog through the reports.save command", async () => {
      await renderView();
      act(() => void runCommand("reports.save"));
      expect(await screen.findByRole("dialog", { name: "Save report" })).toBeInTheDocument();
    });

    it("shows an error toast when the text cannot be built", async () => {
      mocked.reportText.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: /Copy as text/ }));
      expect(await screen.findByText(/Could not build the report: Disk full/)).toBeInTheDocument();
      expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    });
  });

  describe("copy notes", () => {
    it("copies the notes of the report through the reports.copyNotes command", async () => {
      mocked.reportRun.mockResolvedValue(reportResult({ notes: { text: "Fixed the header\nReview", count: 2 } }));
      await renderView();
      act(() => void runCommand("reports.copyNotes"));
      expect(await screen.findByText("Copied 2 notes")).toBeInTheDocument();
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("Fixed the header\nReview");
    });

    it("copies nothing and says so when the report has no notes", async () => {
      await renderView();
      act(() => void runCommand("reports.copyNotes"));
      expect(await screen.findByText("No notes in the report")).toBeInTheDocument();
      expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
    });

    it("shows an error toast when the report cannot be built", async () => {
      await renderView();
      mocked.reportRun.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      act(() => void runCommand("reports.copyNotes"));
      expect(await screen.findByText(/Could not build the report: Disk full/)).toBeInTheDocument();
    });
  });

  describe("save report", () => {
    it("asks for a name and saves the shown filter under it", async () => {
      await renderView({ reportFilter: { ...defaultFilter(settings()), range: { kind: "lastMonth" } } });
      await userEvent.click(screen.getByRole("button", { name: "Save report…" }));
      const dialog = await screen.findByRole("dialog", { name: "Save report" });
      await userEvent.type(within(dialog).getByRole("textbox", { name: "Name" }), "  Monthly  ");
      await userEvent.click(within(dialog).getByRole("button", { name: "Save" }));
      expect(await screen.findByText('Saved report "Monthly"')).toBeInTheDocument();
      expect(mocked.savedReportSave).toHaveBeenCalledWith("Monthly", expect.objectContaining({ range: { kind: "lastMonth" } }));
      expect(useStore.getState().savedReports.map((r) => r.name)).toEqual(["Monthly"]);
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("saves nothing when the dialog is cancelled", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: "Save report…" }));
      await userEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Cancel" }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mocked.savedReportSave).not.toHaveBeenCalled();
    });

    it("shows an error toast when saving fails", async () => {
      mocked.savedReportSave.mockRejectedValue({ kind: "invalid_input", message: "Name taken", code: null });
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: "Save report…" }));
      await userEvent.type(await screen.findByRole("textbox", { name: "Name" }), "Weekly{Enter}");
      expect(await screen.findByText(/Could not save the report: Name taken/)).toBeInTheDocument();
      expect(useStore.getState().savedReports).toEqual([]);
    });
  });

  describe("data changes", () => {
    it("runs the report again when data changed", async () => {
      await renderView();
      mocked.reportRun.mockResolvedValue(reportResult({ clients: [reportClient({ key: "c2", label: "Globex" })] }));
      act(() => useStore.getState().dataChanged({ kind: "recordings", day: DAY }));
      expect(await screen.findByText("Globex")).toBeInTheDocument();
    });
  });

  describe("saved report", () => {
    it("shows the filter of an opened saved report", async () => {
      await renderView();
      const saved = savedReport({ filter: { ...defaultFilter(null), range: { kind: "lastWeek" }, groupBy: "tag" } });
      act(() => useStore.getState().openSavedReport(saved));
      await vi.waitFor(() => expect(lastFilter()).toEqual(saved.filter));
      expect(screen.getByRole("combobox", { name: "Period" })).toHaveValue("lastWeek");
      await userEvent.click(screen.getByRole("button", { name: "Group copied text by" }));
      expect(screen.getByRole("menuitemcheckbox", { name: "By tag" })).toHaveAttribute("aria-checked", "true");
    });
  });
});
