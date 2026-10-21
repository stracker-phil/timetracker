import { PanelLeft } from "lucide-react";
import { useBindingLabel, runCommand } from "../commands";
import { formatClock, formatHm } from "../lib/format";
import { useStore } from "../state/store";

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

export function StatusBar() {
  const sidebarKeys = useBindingLabel("sidebar.toggle");
  const resetKeys = useBindingLabel("zoom.reset");
  const zoom = useStore((s) => s.settings?.uiFontSize);
  const selection = useStore((s) => s.selectionSnap);
  return (
    <footer className="statusbar">
      <button className="icon-btn status-icon" aria-label="Toggle sidebar" title={sidebarKeys ? `Toggle sidebar (${sidebarKeys})` : "Toggle sidebar"} onClick={() => runCommand("sidebar.toggle")}>
        <PanelLeft size="0.923rem" />
      </button>
      {selection && (
        <span className="mono" aria-label="Selection">
          {formatClock(selection.start)}–{formatClock(selection.end)} · {formatHm(selection.end - selection.start)}
        </span>
      )}
      <span className="spacer" />
      <RunningTimer />
      {zoom !== undefined && (
        <button className="status-text" title={resetKeys ? `Reset zoom (${resetKeys})` : "Reset zoom"} onClick={() => runCommand("zoom.reset")}>
          {zoom} px
        </button>
      )}
    </footer>
  );
}
