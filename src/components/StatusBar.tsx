import { PanelLeft } from "lucide-react";
import { api } from "../api";
import { useBindingLabel, runCommand } from "../commands";
import { today } from "../lib/day";
import { formatClock, formatHm } from "../lib/format";
import { useApiResult } from "../lib/useApiResult";
import { selectDayVersion, useStore } from "../state/store";
import { useLiveReload } from "./day/useLiveReload";
import { TrackerStatus } from "./TrackerStatus";

/** `⏱ Project 1:23`; a click stops the timer. */
function RunningTimer() {
  const running = useStore((s) => s.timer?.running);
  if (!running) return null;
  return (
    <button className="status-text status-timer" title="Stop timer" onClick={() => void useStore.getState().stopTimer()}>
      ⏱ {running.projectName} <span className="mono">{formatHm(running.elapsedMs)}</span>
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
    <span className="mono" aria-label="Today">
      recorded {formatHm(totals.recordedMs)} · tracked {formatHm(totals.trackedMs)}
    </span>
  );
}

export function StatusBar() {
  const sidebarKeys = useBindingLabel("sidebar.toggle");
  const selection = useStore((s) => s.selectionSnap);
  return (
    <footer className="statusbar">
      <button className="icon-btn status-icon" aria-label="Toggle sidebar" title={sidebarKeys ? `Toggle sidebar (${sidebarKeys})` : "Toggle sidebar"} onClick={() => runCommand("sidebar.toggle")}>
        <PanelLeft size="0.923rem" />
      </button>
      <TrackerStatus />
      {selection && (
        <span className="mono" aria-label="Selection">
          {formatClock(selection.start)}–{formatClock(selection.end)} · {formatHm(selection.end - selection.start)}
        </span>
      )}
      <span className="spacer" />
      <RunningTimer />
      <TodayTotals />
    </footer>
  );
}
