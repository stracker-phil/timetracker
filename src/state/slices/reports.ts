import { api, type ReportFilter, type SavedReport, type Settings } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";
import { setSavedReports } from "../../queries/reports";
import { currentSettings } from "../../queries/settings";

export interface ReportsSlice {
  /** `null` until changed: the report then follows `settings.reports`. */
  reportFilter: ReportFilter | null;
  /** The saved report shown, until the filter changes. */
  openedReport: string | null;
  /** Merges `patch` into the shown filter; the Reports view runs it again. */
  setReportFilter: (patch: Partial<ReportFilter>) => void;
  /**
   * Moves the report's period `by` steps of its unit (Rust decides), from the period holding `anchor` when
   * given; `by = 0` with an anchor is the period holding `anchor`.
   */
  shiftReportRange: (anchor: string | null, by: number) => Promise<void>;
  /** Saves the shown filter under `name`. */
  saveReport: (name: string) => Promise<void>;
  renameSavedReport: (id: string, name: string) => Promise<void>;
  deleteSavedReport: (id: string) => Promise<void>;
  /** Shows the Reports view with a saved report's filter. */
  openSavedReport: (saved: SavedReport) => void;
}

export function defaultFilter(settings: Settings | null): ReportFilter {
  return {
    range: { kind: "today" },
    clientIds: [],
    projectIds: [],
    tagIds: [],
    groupBy: settings?.reports.defaultGroup ?? "project",
    rounding: settings?.reports.rounding ?? "none",
    includeArchived: false,
  };
}

export const selectReportFilter = (s: { reportFilter: ReportFilter | null }) => s.reportFilter ?? defaultFilter(currentSettings());

export const reportsSlice: Slice<ReportsSlice> = {
  create: (set, get) => ({
    reportFilter: null,
    openedReport: null,

    setReportFilter(patch) {
      set({ reportFilter: { ...selectReportFilter(get()), ...patch }, openedReport: null });
    },

    async shiftReportRange(anchor, by) {
      try {
        const range = await api.reportRangeShift(selectReportFilter(get()).range, anchor, by);
        get().setReportFilter({ range });
      } catch (e) {
        fail(e, "Could not change the report's period");
      }
    },

    async saveReport(name) {
      try {
        const saved = await api.savedReportSave(name, selectReportFilter(get()));
        setSavedReports((reports) => [...reports, saved]);
        get().toast("success", `Saved report "${saved.name}"`);
      } catch (e) {
        fail(e, "Could not save the report");
      }
    },

    async renameSavedReport(id, name) {
      try {
        const renamed = await api.savedReportRename(id, name);
        setSavedReports((reports) => reports.map((r) => (r.id === id ? renamed : r)));
      } catch (e) {
        fail(e, "Could not rename the report");
      }
    },

    async deleteSavedReport(id) {
      try {
        await api.savedReportDelete(id);
        setSavedReports((reports) => reports.filter((r) => r.id !== id));
        set((s) => ({ openedReport: s.openedReport === id ? null : s.openedReport }));
      } catch (e) {
        fail(e, "Could not delete the report");
      }
    },

    openSavedReport(saved) {
      set({ reportFilter: saved.filter, openedReport: saved.id });
      get().setView("reports");
    },
  }),
};
