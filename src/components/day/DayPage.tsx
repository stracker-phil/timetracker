import { useEffect } from "react";
import type { DayView } from "../../api";
import { isToday } from "../../lib/day";
import { selectDayVersion, useStore } from "../../state/store";
import { Splitter, usePaneSize } from "../Splitter";
import { DayHeader } from "./DayHeader";
import { ListView } from "./list/ListView";
import { Timeline } from "./Timeline";
import { Timesheet } from "./Timesheet";
import { useLiveReload } from "./useLiveReload";

const TIMESHEET = { min: 220, max: 560, share: 0.25 };

const isEmpty = (view: DayView) => view.recordings.length === 0 && view.lanes.every((lane) => lane.blocks.length === 0);

function EmptyDay({ day }: { day: string }) {
  const tracker = useStore((s) => s.tracker);
  if (isToday(day) && tracker?.state.kind === "needsPermission") {
    return (
      <div className="day-empty">
        <p>Tracking needs permission</p>
        <p className="o2">TimeTracker records the app and window in front once macOS allows it.</p>
        <button type="button" className="btn btn-primary" onClick={() => useStore.getState().setPermissionDialogHidden(false)}>
          Allow…
        </button>
      </div>
    );
  }
  return (
    <div className="day-empty">
      <p>Nothing tracked on this day</p>
      {isToday(day) && tracker && <p className="o2">{tracker.text}</p>}
    </div>
  );
}

/**
 * The Day view: the timesheet on the left (resizable), then the shown day's timeline or list; it reloads when the
 * day's data changes, and keeps following today.
 */
export function DayPage() {
  const day = useStore((s) => s.day);
  const version = useStore(selectDayVersion(day));
  const live = useLiveReload(isToday(day));
  const view = useStore((s) => (s.dayView?.day === day ? s.dayView : null));
  const error = useStore((s) => s.dayError);
  const mode = useStore((s) => s.uiState.dayMode);
  const pane = usePaneSize("timesheet", Math.round(window.innerWidth * TIMESHEET.share), TIMESHEET.min, TIMESHEET.max);

  useEffect(() => {
    void useStore.getState().loadDay();
  }, [day, version, live]);

  return (
    <section className="day-page" aria-label="Day">
      <DayHeader day={day} totals={view?.totals ?? null} />
      {error && !view && <p className="settings-alert">{error.message}</p>}
      {view && (
        <div className="day-body">
          <div className="timesheet-pane" style={{ width: pane.size }}>
            <Timesheet view={view} />
          </div>
          <Splitter axis="x" paneAfter={false} label="Resize the timesheet" {...pane} />
          <div className="day-main">{isEmpty(view) ? <EmptyDay day={day} /> : mode === "list" ? <ListView day={day} reload={live} /> : <Timeline view={view} />}</div>
        </div>
      )}
    </section>
  );
}
