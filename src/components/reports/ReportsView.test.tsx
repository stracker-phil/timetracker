import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ReportFilter, type Rounding } from "../../api";
import { installCommands, runCommand } from "../../commands";
import { formatDayLabel } from "../../lib/day";
import { defaultFilter } from "../../state/slices/reports";
import { resetStore, useStore } from "../../state/store";
import { at, DAY } from "../../test/dayFixtures";
import { projectItem, projectsStore, tag } from "../../test/projectFixtures";
import { reportGroup, reportResult, reportRow, savedReport } from "../../test/reportFixtures";
import { settings } from "../../test/settings";
import { Dialogs } from "../Dialogs";
import { Toasts } from "../Toasts";
import { ReportsView } from "./ReportsView";

const mocked = vi.mocked(api, true);

const lastFilter = (): ReportFilter => mocked.reportRun.mock.lastCall![0];
const total = () => within(screen.getByText("Total").parentElement!);

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
  await screen.findByRole("heading", { level: 1 });
}

describe("ReportsView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.reportRun.mockResolvedValue(reportResult());
  });

  describe("result", () => {
    it("titles a one-day report with the day and shows each group with its hours and the total", async () => {
      mocked.reportRun.mockResolvedValue(
        reportResult({
          groups: [reportGroup(), reportGroup({ key: "p2", label: "Docs", durationMs: 1_800_000, roundedMs: 1_800_000, rows: [] })],
          totalMs: 4_500_000,
          totalRoundedMs: 4_500_000,
        }),
      );
      await renderView();
      expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(formatDayLabel(DAY));
      expect(within(screen.getByRole("region", { name: "Website" })).getByText("0:45 h")).toBeInTheDocument();
      expect(within(screen.getByRole("region", { name: "Docs" })).getByText("0:30 h")).toBeInTheDocument();
      expect(total().getByText("1:15 h")).toBeInTheDocument();
    });

    it("titles a multi-day report with its first and last day", async () => {
      mocked.reportRun.mockResolvedValue(reportResult({ from: "2026-03-09", to: "2026-03-15" }));
      await renderView();
      expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("2026-03-09 – 2026-03-15");
    });

    it("lists the notes of a group", async () => {
      mocked.reportRun.mockResolvedValue(reportResult({ groups: [reportGroup({ notes: [{ text: "Fixed the header", days: [DAY] }, { text: "Review", days: [DAY] }] })] }));
      await renderView();
      expect(screen.getByText("Fixed the header")).toBeInTheDocument();
      expect(screen.getByText("Review")).toBeInTheDocument();
    });

    it("says there are no recordings and shows no total when the report is empty", async () => {
      mocked.reportRun.mockResolvedValue(reportResult({ groups: [], totalMs: 0, totalRoundedMs: 0 }));
      await renderView();
      expect(await screen.findByText("No recordings.")).toBeInTheDocument();
      expect(screen.queryByText("Total")).not.toBeInTheDocument();
    });

    it("shows no tag overlap caveat, as a recording has at most one tag", async () => {
      await renderView();
      await screen.findByText("Total");
      expect(screen.queryByText(/Tags overlap/)).not.toBeInTheDocument();
    });

    it("shows the unrounded hours muted next to rounded ones when rounding changed them", async () => {
      mocked.reportRun.mockResolvedValue(reportResult({ groups: [reportGroup({ roundedMs: 3_600_000 })], totalRoundedMs: 3_600_000 }));
      await renderView({ reportFilter: { ...defaultFilter(settings()), rounding: "up30" } });
      expect(await total().findByText("1:00 h")).toBeInTheDocument();
      expect(total().getByText("0:45")).toBeInTheDocument();
    });

    it("shows only the rounded hours when no rounding is on", async () => {
      await renderView();
      await screen.findByText("Total");
      expect(total().queryByTitle("Before rounding")).not.toBeInTheDocument();
    });

    it("shows the error when the report fails", async () => {
      mocked.reportRun.mockRejectedValue({ kind: "io", message: "Database is locked", code: null });
      resetStore({ settings: settings(), view: "reports" });
      render(<ReportsView />);
      expect(await screen.findByText("Database is locked")).toBeInTheDocument();
    });

    it("reveals the recordings of a group on request", async () => {
      mocked.reportRun.mockResolvedValue(reportResult({ groups: [reportGroup({ rows: [reportRow({ note: "Fixed the header", client: "Acme", tag: "support" })] })] }));
      await renderView();
      const toggle = await screen.findByRole("button", { name: "1 recording" });
      expect(toggle).toHaveAttribute("aria-expanded", "false");
      expect(screen.queryByRole("table")).not.toBeInTheDocument();
      await userEvent.click(toggle);
      expect(toggle).toHaveAttribute("aria-expanded", "true");
      const table = screen.getByRole("table");
      expect(within(table).getByText("09:00–09:45")).toBeInTheDocument();
      expect(within(table).getByText("Acme / Website")).toBeInTheDocument();
      expect(within(table).getByText("support")).toBeInTheDocument();
      expect(within(table).getByText("Fixed the header")).toBeInTheDocument();
    });

    it("shows every span of a recording in one row, with the total duration", async () => {
      const spans = [
        { start: at("14:25"), end: at("15:00") },
        { start: at("16:05"), end: at("20:15") },
      ];
      mocked.reportRun.mockResolvedValue(
        reportResult({ groups: [reportGroup({ rows: [reportRow({ start: spans[0].start, end: spans[1].end, spans, durationMs: 285 * 60_000, roundedMs: 285 * 60_000 })] })] }),
      );
      await renderView();
      await userEvent.click(await screen.findByRole("button", { name: "1 recording" }));
      const row = within(screen.getByRole("table")).getAllByRole("row")[1];
      expect(within(row).getByText("14:25–15:00, 16:05–20:15")).toBeInTheDocument();
      expect(within(row).getByText("4:45")).toBeInTheDocument();
    });

    it("counts the recordings of a group on its toggle", async () => {
      mocked.reportRun.mockResolvedValue(reportResult({ groups: [reportGroup({ rows: [reportRow(), reportRow({ id: "r2" })] })] }));
      await renderView();
      expect(await screen.findByRole("button", { name: "2 recordings" })).toBeInTheDocument();
    });
  });

  describe("filters", () => {
    it("runs today's report grouped and rounded as the settings say until the filter is changed", async () => {
      resetStore({ settings: settings({ reports: { rounding: "nearest15", defaultGroup: "week" } }), view: "reports" });
      render(<ReportsView />);
      await screen.findByText("Total");
      expect(lastFilter()).toEqual({ ...defaultFilter(null), range: { kind: "today" }, groupBy: "week", rounding: "nearest15" });
    });

    it("runs the report again with the chosen range", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: "Last week" }));
      await vi.waitFor(() => expect(lastFilter().range).toEqual({ kind: "lastWeek" }));
      expect(screen.getByRole("button", { name: "Last week" })).toHaveAttribute("aria-pressed", "true");
    });

    it("runs the report again with the chosen grouping", async () => {
      await renderView();
      const group = screen.getByRole("group", { name: "Group by" });
      await userEvent.click(within(group).getByRole("button", { name: "Tag" }));
      await vi.waitFor(() => expect(lastFilter().groupBy).toBe("tag"));
      expect(within(group).getByRole("button", { name: "Tag" })).toHaveAttribute("aria-pressed", "true");
    });

    it.each<[string, Rounding]>([
      ["Nearest 15 min", "nearest15"],
      ["Up to 30 min", "up30"],
      ["No rounding", "none"],
    ])("runs the report again with the %s rounding", async (label, value) => {
      await renderView({ reportFilter: { ...defaultFilter(settings()), rounding: value === "none" ? "up5" : "none" } });
      await userEvent.selectOptions(screen.getByRole("combobox", { name: "Rounding" }), label);
      await vi.waitFor(() => expect(lastFilter().rounding).toBe(value));
    });

    it("runs the report again with archived projects included", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("switch", { name: "Archived projects" }));
      await vi.waitFor(() => expect(lastFilter().includeArchived).toBe(true));
    });

    it("starts a custom range from the days shown", async () => {
      mocked.reportRun.mockResolvedValue(reportResult({ from: "2026-03-09", to: "2026-03-15" }));
      await renderView();
      await userEvent.click(await screen.findByRole("button", { name: "Custom" }));
      expect(await screen.findByLabelText("From")).toHaveValue("2026-03-09");
      expect(screen.getByLabelText("To")).toHaveValue("2026-03-15");
      await vi.waitFor(() => expect(lastFilter().range).toEqual({ kind: "custom", from: "2026-03-09", to: "2026-03-15" }));
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
      await screen.findByText("Total");
      mocked.reportRun.mockResolvedValue(reportResult({ groups: [reportGroup({ label: "Docs", key: "p2" })] }));
      act(() => useStore.getState().dataChanged({ kind: "recordings", day: DAY }));
      expect(await screen.findByRole("region", { name: "Docs" })).toBeInTheDocument();
    });
  });

  describe("saved report", () => {
    it("shows the filter of an opened saved report", async () => {
      await renderView();
      const saved = savedReport({ filter: { ...defaultFilter(null), range: { kind: "lastWeek" }, groupBy: "tag" } });
      act(() => useStore.getState().openSavedReport(saved));
      await vi.waitFor(() => expect(lastFilter()).toEqual(saved.filter));
      expect(screen.getByRole("button", { name: "Last week" })).toHaveAttribute("aria-pressed", "true");
    });
  });
});
