import { Copy, Save } from "lucide-react";
import { api, type ReportResult } from "../../api";
import { useBindingLabel } from "../../commands";
import { formatDayLabel } from "../../lib/day";
import { formatHm } from "../../lib/format";
import { useApiResult } from "../../lib/useApiResult";
import { defaultFilter } from "../../state/slices/reports";
import { useStore } from "../../state/store";
import { copyReport, promptSaveReport } from "./actions";
import { ReportFilters } from "./ReportFilters";
import { ReportGroupTable } from "./ReportGroupTable";

const reportTitle = (result: ReportResult) => (result.from === result.to ? formatDayLabel(result.from) : `${result.from} – ${result.to}`);

/** The filter bar and the report it gives; it runs again whenever the filter or the data changes. */
export function ReportsView() {
  const reportFilter = useStore((s) => s.reportFilter);
  const settings = useStore((s) => s.settings);
  const changes = useStore((s) => s.changeCount);
  const setReportFilter = useStore((s) => s.setReportFilter);
  const filter = reportFilter ?? defaultFilter(settings);
  const copyKeys = useBindingLabel("reports.copyText");
  const key = settings ? `${JSON.stringify(filter)}#${changes}` : null;
  const { data: result, error } = useApiResult(key, () => api.reportRun(filter), { keepData: true });
  const rounded = filter.rounding !== "none";
  const shown = result ? { from: result.from, to: result.to } : null;

  return (
    <section className="reports-page" aria-label="Reports">
      <header className="reports-head">
        <h1 className="reports-title">{result && reportTitle(result)}</h1>
        <button type="button" className="btn" title={copyKeys ? `Copy as text (${copyKeys})` : undefined} onClick={() => void copyReport()}>
          <Copy size="1rem" /> Copy as text
        </button>
        <button type="button" className="btn" onClick={() => promptSaveReport()}>
          <Save size="1rem" /> Save report…
        </button>
      </header>
      <ReportFilters filter={filter} shown={shown} onChange={setReportFilter} />
      <div className="report-result">
        {error && <p className="settings-alert">{error.message}</p>}
        {result && result.groups.length === 0 && <p className="o2">No recordings.</p>}
        {result?.groups.map((group) => (
          <ReportGroupTable key={group.key} group={group} rounded={rounded} />
        ))}
        {result && result.groups.length > 0 && (
          <footer className="report-total">
            <span className="grow-text">Total</span>
            {rounded && result.totalRoundedMs !== result.totalMs && <span className="mono o3" title="Before rounding">{formatHm(result.totalMs)}</span>}
            <span className="mono report-hours">{formatHm(result.totalRoundedMs)} h</span>
          </footer>
        )}
        {result?.billable.percent != null && (
          <footer className="report-billable" aria-label="Billable total">
            <span className="grow-text">Billable</span>
            <span className="mono report-hours">
              {formatHm(result.billable.billableMs)} h · {result.billable.percent} %
            </span>
          </footer>
        )}
      </div>
    </section>
  );
}
