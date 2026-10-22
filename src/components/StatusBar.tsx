import { PanelLeft } from "lucide-react";
import { api, type Range } from "../api";
import { useBindingLabel, runCommand } from "../commands";
import { today } from "../lib/day";
import { formatClock, formatHm } from "../lib/format";
import { useApiResult } from "../lib/useApiResult";
import { selectDayVersion, useStore } from "../state/store";
import { useLiveReload } from "./day/useLiveReload";
import { useCalendarPeriod, useReportRange } from "./shownPeriod";
import { TrackerStatus } from "./TrackerStatus";

/** `⏱ Project 1:23`; a click stops the timer. */
function RunningTimer() {
  const running = useStore((s) => s.timer?.running);
  if (!running) return null;
  return (
    <button className="status-text status-timer" title="Stop timer" onClick={() => void useStore.getState().stopTimer()}>
      ⏱ {running.projectName} <span className="tabular">{formatHm(running.elapsedMs)}</span>
    </button>
  );
}

/** Today's recorded and tracked totals, shown in every view. */
function TodayTotals() {
  const tick = useLiveReload(true);
  const day = today();
  const version = useStore(selectDayVersion(day));
  const { data: totals } = useApiResult(`${day}#${version}#${tick}`, () => api.dayTotals(day), { keepData: true });
  if (!totals) return null;
  return (
    <span className="tabular" aria-label="Today">
      recorded {formatHm(totals.recordedMs)} · tracked {formatHm(totals.trackedMs)}
    </span>
  );
}

const TODAY: Range = { kind: "today" };

/** The period the shown view covers; today in views without one; `null` while it loads. */
function useShownRange(): Range | null {
  const view = useStore((s) => s.view);
  const reportRange = useReportRange();
  const calendar = useCalendarPeriod();
  if (view === "reports") return reportRange;
  if (view !== "day" && view !== "week" && view !== "month") return TODAY;
  if (!calendar) return null;
  return { kind: "custom", from: calendar.period.from, to: calendar.period.to };
}

/** `Billable 80 %` with a bar, for the shown period; hidden while nothing is recorded in it. */
function BillableRatio() {
  const range = useShownRange();
  if (!range) return null;
  return <BillableMeter range={range} />;
}

function BillableMeter({ range }: { range: Range }) {
  const tick = useLiveReload(true);
  const changes = useStore((s) => s.changeCount);
  const { data: billable } = useApiResult(`${JSON.stringify(range)}#${changes}#${tick}`, () => api.billableRatio(range), { keepData: true });
  if (billable?.percent == null) return null;
  return (
    <span
      className="status-billable"
      role="meter"
      aria-label="Billable"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={billable.percent}
      title={`Billable ${formatHm(billable.billableMs)} / ${formatHm(billable.recordedMs)}`}
    >
      <span className="status-billable-track" aria-hidden="true">
        <span className="status-billable-fill" style={{ width: `${billable.percent}%` }} />
      </span>
      Billable {billable.percent} %
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
          {formatClock(selection.start)}–{formatClock(selection.end)} · {formatHm(selection.end - selection.start)}
        </span>
      )}
      <span className="spacer" />
      <RunningTimer />
      <BillableRatio />
      <TodayTotals />
    </footer>
  );
}
