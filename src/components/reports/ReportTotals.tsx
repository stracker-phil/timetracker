import type { ReportResult } from "../../api";
import { formatCount, formatDuration } from "../../lib/format";

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="report-stat">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

/** The period's total, large, beside the counts, the untagged time and the billable share. */
export function ReportTotals({ result }: { result: ReportResult }) {
  const { billable } = result;
  const roundedAway = result.filter.rounding !== "none" && result.totalRoundedMs !== result.totalMs;
  return (
    <dl className="report-totals" aria-label="Totals">
      <div className="report-stat report-stat--total">
        <dt>Total</dt>
        <dd>{formatDuration(result.totalRoundedMs)}</dd>
        {roundedAway && <dd className="report-stat-note">{formatDuration(result.totalMs)} before rounding</dd>}
      </div>
      <Stat label="Recordings" value={formatCount(result.recordingCount)} />
      <Stat label="Projects" value={formatCount(result.projectCount)} />
      {result.untaggedRoundedMs > 0 && <Stat label="Untagged" value={formatDuration(result.untaggedRoundedMs)} />}
      {billable.percent != null && <Stat label="Billable" value={`${formatDuration(billable.billableMs)} · ${billable.percent} %`} />}
    </dl>
  );
}
