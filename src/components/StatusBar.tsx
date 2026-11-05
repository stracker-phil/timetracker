import { PanelLeft } from "lucide-react";
import type { Range } from "../api";
import { useBindingLabel, runCommand } from "../commands";
import { formatClock, formatDuration } from "../lib/format";
import { NO_PROJECT } from "../state/slices/suggestions";
import { useStore } from "../state/store";
import { useLiveReload } from "./day/useLiveReload";
import { useCalendarPeriod, useReportRange } from "./shownPeriod";
import { TrackerStatus } from "./TrackerStatus";
import { useTimer } from "../queries/status";
import { PERIOD_TOTALS_KEY, usePeriodTotals } from "../queries/calendar";

/** `⏱ Project 1:23`, with `· Suggested` while another app's timer waits for review; a click stops the timer. */
function RunningTimer() {
  const running = useTimer()?.running;
  if (!running) return null;
  return (
    <button className="status-text status-timer" title="Stop timer" onClick={() => void useStore.getState().stopTimer()}>
      ⏱ {running.projectName ?? NO_PROJECT} <span className="tabular">{formatDuration(running.elapsedMs)}</span>
      {running.suggested && " · Suggested"}
    </button>
  );
}

/** The period the shown view covers: the report's range, or the Day, Week or Month view's days; null elsewhere and while it loads. */
function useShownRange(): Range | null {
  const view = useStore((s) => s.view);
  const reportRange = useReportRange();
  const calendar = useCalendarPeriod();
  if (view === "reports") return reportRange;
  if (!calendar) return null;
  return { kind: "custom", from: calendar.period.from, to: calendar.period.to };
}

/** `This week · recorded 1:30 · Billable 67 %` for the shown period; nothing in views without one. */
function ShownPeriodTotals() {
  const range = useShownRange();
  if (!range) return null;
  return <PeriodNumbers range={range} />;
}

const LIVE_KEYS = [PERIOD_TOTALS_KEY];

function PeriodNumbers({ range }: { range: Range }) {
  useLiveReload(true, LIVE_KEYS);
  const { data: totals } = usePeriodTotals(range);
  if (!totals) return null;
  const percent = totals.billablePercent;
  return (
    <span className="status-period tabular" aria-label="Shown period">
      {totals.label} · recorded {formatDuration(totals.recordedMs)}
      {percent != null && (
        <>
          {" · "}
          <span
            className="status-billable"
            role="meter"
            aria-label="Billable"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent}
            title={`Billable ${formatDuration(totals.billableMs)} / ${formatDuration(totals.recordedMs)}`}
          >
            <span className="status-billable-track" aria-hidden="true">
              <span className="status-billable-fill" style={{ width: `${percent}%` }} />
            </span>
            Billable {percent} %
          </span>
        </>
      )}
    </span>
  );
}

export function StatusBar() {
  const sidebarKeys = useBindingLabel("sidebar.toggle");
  const selection = useStore((s) => s.selectionSnap);
  // The sidebar holds the tracker's status while it shows.
  const sidebarHidden = useStore((s) => s.uiState.sidebarHidden);
  return (
    <footer className="statusbar">
      <button className="icon-btn status-icon" aria-label="Toggle sidebar" title={sidebarKeys ? `Toggle sidebar (${sidebarKeys})` : "Toggle sidebar"} onClick={() => runCommand("sidebar.toggle")}>
        <PanelLeft size="0.923rem" />
      </button>
      {sidebarHidden && <TrackerStatus />}
      {selection && (
        <span className="tabular" aria-label="Selection">
          {formatClock(selection.start)}–{formatClock(selection.end)} · {formatDuration(selection.end - selection.start)}
        </span>
      )}
      <span className="spacer" />
      <RunningTimer />
      <ShownPeriodTotals />
    </footer>
  );
}
