import { Plus } from "lucide-react";
import type { DayView } from "../../api";
import { runCommand, useBindingLabel } from "../../commands";
import { formatShortDuration } from "../../lib/format";
import { useStore } from "../../state/store";
import { TimesheetCard } from "./TimesheetCard";

/** The left pane of the Day view: the day's recorded total, + for a new recording, and one card per recording in order. */
export function Timesheet({ view }: { view: DayView }) {
  const focused = useStore((s) => s.focusedRecording);
  const hovered = useStore((s) => s.hoveredRecording);
  const newKeys = useBindingLabel("recording.new");
  return (
    <aside className="timesheet" aria-label="Timesheet">
      <header className="timesheet-head">
        <div className="timesheet-heading">
          <span className="timesheet-label">Timesheet</span>
          <span className="timesheet-total" aria-label="Recorded">
            {formatShortDuration(view.totals.recordedMs)}
          </span>
        </div>
        <button type="button" className="btn timesheet-add" aria-label="New recording" title={newKeys ? `New recording (${newKeys})` : "New recording"} onClick={() => runCommand("recording.new")}>
          <Plus size="1.077rem" aria-hidden="true" />
        </button>
      </header>
      <div className="timesheet-cards">
        {view.timesheet.length === 0 && <p className="timesheet-empty o3">Nothing recorded yet. Drag over the timeline, or press +.</p>}
        {view.timesheet.map((card) => (
          <TimesheetCard key={card.id} card={card} focused={card.id === focused} hovered={card.id === hovered} />
        ))}
      </div>
    </aside>
  );
}
