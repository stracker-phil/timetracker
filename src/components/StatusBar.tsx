import { PanelLeft } from "lucide-react";
import { api, type Range } from "../api";
import { useBindingLabel, runCommand } from "../commands";
import { formatClock, formatDuration } from "../lib/format";
import { useApiResult } from "../lib/useApiResult";
import { useStore } from "../state/store";
import { useLiveReload } from "./day/useLiveReload";
import { useCalendarPeriod, useReportRange } from "./shownPeriod";
import { TrackerStatus } from "./TrackerStatus";

/** `⏱ Project 1:23`; a click stops the timer. */
function RunningTimer() {
  const running = useStore((s) => s.timer?.running);
  if (!running) return null;
  return (
    <button className="status-text status-timer" title="Stop timer" onClick={() => void useStore.getState().stopTimer()}>
      ⏱ {running.projectName} <span className="tabular">{formatDuration(running.elapsedMs)}</span>
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

function PeriodNumbers({ range }: { range: Range }) {
  const tick = useLiveReload(true);
  const changes = useStore((s) => s.changeCount);
  const { data: totals } = useApiResult(`${JSON.stringify(range)}#${changes}#${tick}`, () => api.periodTotals(range), { keepData: true });
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
