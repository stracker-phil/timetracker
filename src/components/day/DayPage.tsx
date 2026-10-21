import { useEffect } from "react";
import type { DayView } from "../../api";
import { isToday } from "../../lib/day";
import { selectDayVersion, useStore } from "../../state/store";
import { DayHeader } from "./DayHeader";
import { RecordingEditor } from "./RecordingEditor";
import { Timeline } from "./Timeline";
import { useLiveReload } from "./useLiveReload";

const isEmpty = (view: DayView) => view.away.length === 0 && view.recordings.length === 0 && view.lanes.every((lane) => lane.blocks.length === 0);

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

/** The Day view: the shown day's timeline; it reloads when the day's data changes, and keeps following today. */
export function DayPage() {
  const day = useStore((s) => s.day);
  const version = useStore(selectDayVersion(day));
  const live = useLiveReload(isToday(day));
  const view = useStore((s) => (s.dayView?.day === day ? s.dayView : null));
  const error = useStore((s) => s.dayError);
  const editor = useStore((s) => s.editor);

  useEffect(() => {
    void useStore.getState().loadDay();
  }, [day, version, live]);

  return (
    <section className="day-page" aria-label="Day">
      <DayHeader day={day} totals={view?.totals ?? null} />
      {error && !view && <p className="settings-alert">{error.message}</p>}
      {view && (isEmpty(view) ? <EmptyDay day={day} /> : <Timeline view={view} />)}
      {editor && <RecordingEditor key={editor.mode === "edit" ? editor.id : `${editor.start}`} editor={editor} />}
    </section>
  );
}
