import { api, type ReportFilter, type SavedReport, type Settings } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface ReportsSlice {
  /** `null` until changed: the report then follows `settings.reports`. */
  reportFilter: ReportFilter | null;
  savedReports: SavedReport[];
  /** Merges `patch` into the shown filter; the Reports view runs it again. */
  setReportFilter: (patch: Partial<ReportFilter>) => void;
  loadSavedReports: () => Promise<void>;
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

export const selectReportFilter = (s: { reportFilter: ReportFilter | null; settings: Settings | null }) => s.reportFilter ?? defaultFilter(s.settings);

export const reportsSlice: Slice<ReportsSlice> = {
  create: (set, get) => ({
    reportFilter: null,
    savedReports: [],

    setReportFilter(patch) {
      set({ reportFilter: { ...selectReportFilter(get()), ...patch } });
    },

    async loadSavedReports() {
      try {
        set({ savedReports: await api.savedReportsList() });
      } catch (e) {
        fail(e, "Could not load the saved reports");
      }
    },

    async saveReport(name) {
      try {
        const saved = await api.savedReportSave(name, selectReportFilter(get()));
        set((s) => ({ savedReports: [...s.savedReports, saved] }));
        get().toast("success", `Saved report "${saved.name}"`);
      } catch (e) {
        fail(e, "Could not save the report");
      }
    },

    async renameSavedReport(id, name) {
      try {
        const renamed = await api.savedReportRename(id, name);
        set((s) => ({ savedReports: s.savedReports.map((r) => (r.id === id ? renamed : r)) }));
      } catch (e) {
        fail(e, "Could not rename the report");
      }
    },

    async deleteSavedReport(id) {
      try {
        await api.savedReportDelete(id);
        set((s) => ({ savedReports: s.savedReports.filter((r) => r.id !== id) }));
      } catch (e) {
        fail(e, "Could not delete the report");
      }
    },

    openSavedReport(saved) {
      set({ reportFilter: saved.filter });
      get().setView("reports");
    },
  }),
};
