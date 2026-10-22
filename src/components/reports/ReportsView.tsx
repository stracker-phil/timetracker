import { Copy, Save } from "lucide-react";
import { api, type Group, type ReportResult } from "../../api";
import { useBindingLabel } from "../../commands";
import { formatRangeLabel } from "../../lib/day";
import { useApiResult } from "../../lib/useApiResult";
import { defaultFilter } from "../../state/slices/reports";
import { useStore } from "../../state/store";
import { copyReport, promptSaveReport } from "./actions";
import { ReportChart } from "./ReportChart";
import { ReportFilters } from "./ReportFilters";
import { ReportTable } from "./ReportTable";
import { ReportTotals } from "./ReportTotals";

const TEXT_GROUPS: readonly Group[] = ["project", "tag", "day", "week", "month"];

function ReportBody({ result }: { result: ReportResult }) {
  if (result.clients.length === 0) {
    return (
      <>
        <ReportTotals result={result} />
        <p className="o3">No recordings.</p>
      </>
    );
  }
  return (
    <>
      <ReportTotals result={result} />
      <div className="report-charts">
        <ReportChart charts={result.charts} totalMs={result.totalRoundedMs} initial="client" />
        <ReportChart charts={result.charts} totalMs={result.totalRoundedMs} initial="tag" />
      </div>
      <ReportTable clients={result.clients} />
    </>
  );
}

/** The period's report under a slim filter bar; it runs again whenever the filter or the data changes. */
export function ReportsView() {
  const reportFilter = useStore((s) => s.reportFilter);
  const settings = useStore((s) => s.settings);
  const changes = useStore((s) => s.changeCount);
  const setReportFilter = useStore((s) => s.setReportFilter);
  const filter = reportFilter ?? defaultFilter(settings);
  const copyKeys = useBindingLabel("reports.copyText");
  const key = settings ? `${JSON.stringify(filter)}#${changes}` : null;
  const { data: result, error } = useApiResult(key, () => api.reportRun(filter), { keepData: true });
  const shown = result ? { from: result.from, to: result.to } : null;

  return (
    <section className="reports-page" aria-label="Reports">
      <header className="reports-head">
        <h1 className="reports-title">{result && formatRangeLabel(result.from, result.to)}</h1>
        <select className="input" aria-label="Group copied text by" value={filter.groupBy} onChange={(e) => setReportFilter({ groupBy: e.target.value as Group })}>
          {TEXT_GROUPS.map((group) => (
            <option key={group} value={group}>
              By {group}
            </option>
          ))}
        </select>
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
        {result && <ReportBody result={result} />}
      </div>
    </section>
  );
}
