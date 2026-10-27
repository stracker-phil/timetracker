import { Save, SlidersHorizontal } from "lucide-react";
import { api, type ReportResult } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { defaultFilter } from "../../state/slices/reports";
import { useStore } from "../../state/store";
import { Toolbar } from "../Toolbar";
import { promptSaveReport } from "./actions";
import { CopyTextButton } from "./CopyTextButton";
import { ReportChart } from "./ReportChart";
import { ReportFilters } from "./ReportFilters";
import { ReportTable } from "./ReportTable";
import { ReportTotals } from "./ReportTotals";

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
      <ReportTable clients={result.clients} multiDay={result.from !== result.to} />
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
  const key = settings ? `${JSON.stringify(filter)}#${changes}` : null;
  const { data: result, error } = useApiResult(key, () => api.reportRun(filter), { keepData: true });
  const shown = result ? { from: result.from, to: result.to } : null;

  return (
    <section className="reports-page" aria-label="Reports">
      <Toolbar title={result?.title}>
        <button
          type="button"
          className="icon-btn"
          aria-label="Report options"
          aria-haspopup="dialog"
          title="Report options"
          data-report-options
          onClick={() => useStore.getState().openDialog({ type: "reportOptions" })}
        >
          <SlidersHorizontal size="1.154rem" />
        </button>
        <CopyTextButton groupBy={filter.groupBy} onGroupBy={(groupBy) => setReportFilter({ groupBy })} />
        <button type="button" className="btn" onClick={() => promptSaveReport()}>
          <Save size="1.154rem" /> Save report…
        </button>
      </Toolbar>
      <ReportFilters filter={filter} shown={shown} onChange={setReportFilter} />
      <div className="report-result">
        {error && <p className="settings-alert">{error.message}</p>}
        {result && <ReportBody result={result} />}
      </div>
    </section>
  );
}
