import { keepPreviousData, queryOptions, useQuery } from "@tanstack/react-query";
import { api, type ReportFilter, type SavedReport } from "../api";
import { queryClient } from "./queryClient";

export const REPORT_KEY = ["report"] as const;
export const SAVED_REPORTS_KEY = ["savedReports"] as const;

const NONE: SavedReport[] = [];

/** The report of `filter`; null asks nothing. The last report stays while the next one runs. */
export function useReport(filter: ReportFilter | null) {
  return useQuery({ queryKey: [...REPORT_KEY, filter], queryFn: () => api.reportRun(filter as ReportFilter), enabled: filter !== null, placeholderData: keepPreviousData });
}

export const savedReportsQuery = queryOptions({ queryKey: SAVED_REPORTS_KEY, queryFn: api.savedReportsList, gcTime: Infinity, meta: { failure: "Could not load the saved reports" } });

export const useSavedReports = (): SavedReport[] => useQuery(savedReportsQuery).data ?? NONE;

/** The saved reports as loaded, for code outside components. */
export const savedReports = (): SavedReport[] => queryClient.getQueryData(SAVED_REPORTS_KEY) ?? NONE;

/** Writes the list as a write left it. */
export const setSavedReports = (update: (reports: SavedReport[]) => SavedReport[]) => queryClient.setQueryData(SAVED_REPORTS_KEY, (reports: SavedReport[] | undefined) => update(reports ?? NONE));
