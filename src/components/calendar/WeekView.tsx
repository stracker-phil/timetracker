import { useId, type CSSProperties, type MouseEvent } from "react";
import { api, type DayColumn as Column, type RecordingBlock as Block } from "../../api";
import { formatHm } from "../../lib/format";
import { useApiResult } from "../../lib/useApiResult";
import { useStore } from "../../state/store";
import { RecordingBlock } from "../day/RecordingBlock";
import { RecordingEditor } from "../day/RecordingEditor";
import { Modal } from "../Modal";
import { TimeAxis } from "../day/TimeAxis";
import { CalendarHeader } from "./CalendarHeader";

const MINUTE_MS = 60_000;
const NEW_RECORDING_MS = 60 * MINUTE_MS;

/** The day column under the pointer, if any. */
function dayAt(e: PointerEvent): string | null {
  const el = document.elementFromPoint(e.clientX, e.clientY);
  return el?.closest<HTMLElement>("[data-day]")?.dataset.day ?? null;
}

/** The instant under the pointer, down to the snap grid. */
function startAt(e: MouseEvent<HTMLElement>, origin: number, pxPerMinute: number, snapMinutes: number): number {
  const minutes = (e.clientY - e.currentTarget.getBoundingClientRect().top) / pxPerMinute;
  return origin + Math.floor(minutes / snapMinutes) * snapMinutes * MINUTE_MS;
}

function DayColumn({ column, pxPerMinute, focused }: { column: Column; pxPerMinute: number; focused: string | null }) {
  const [origin] = column.bounds;
  const y = (ms: number) => ((ms - origin) / MINUTE_MS) * pxPerMinute;
  const snapMinutes = useStore((s) => s.settings?.timeline.snapMinutes);
  const create = (e: MouseEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget || !snapMinutes) return;
    const start = startAt(e, origin, pxPerMinute, snapMinutes);
    useStore.getState().openEditor({ mode: "create", start, end: start + NEW_RECORDING_MS });
  };
  // Dropped on another day it moves there, with ⌥ it is copied; on its own day it moves in time.
  const onDrop = (block: Block, e: PointerEvent): boolean => {
    const day = dayAt(e);
    if (!day || day === column.day) return false;
    const { moveRecording, copyRecording } = useStore.getState();
    void (e.altKey ? copyRecording(block.id, day, block.spanId) : moveRecording(block.id, day, block.spanId));
    return true;
  };
  return (
    <div className={column.isToday ? "week-column week-column--today" : "week-column"} data-day={column.day} aria-label={column.label} onDoubleClick={create}>
      {column.recordings.map((r) => (
        <RecordingBlock key={r.spanId} block={r} y={y} pxPerMinute={pxPerMinute} focused={r.id === focused} onDrop={onDrop} />
      ))}
    </div>
  );
}

/** Seven days of recordings side by side; drag a recording to another day to move it, with ⌥ to copy it. */
export function WeekView() {
  const day = useStore((s) => s.day);
  const changes = useStore((s) => s.changeCount);
  const pxPerMinute = useStore((s) => s.uiState.timelineZoom);
  const focused = useStore((s) => s.focusedRecording);
  const editor = useStore((s) => s.editor);
  const editorId = useId();
  const { data: week, error } = useApiResult(`week:${day}#${changes}`, () => api.weekView(day), { keepData: true });
  const goToDay = useStore.getState().goToDay;
  const bounds = week?.days[0]?.bounds;
  const bodyStyle = bounds && ({ height: ((bounds[1] - bounds[0]) / MINUTE_MS) * pxPerMinute, "--hour-px": `${60 * pxPerMinute}px` } as CSSProperties);

  return (
    <section className="day-page" aria-label="Week">
      <CalendarHeader title={week?.label ?? ""}>
        {week && (
          <span className="day-totals mono o2" aria-label="Totals">
            recorded {formatHm(week.totalMs)} · tracked {formatHm(week.trackedMs)}
          </span>
        )}
      </CalendarHeader>
      {error && !week && <p className="settings-alert">{error.message}</p>}
      {week && bounds && (
        <div className="timeline week">
          <div className="week-head" role="row">
            <span aria-hidden="true" />
            {week.days.map((d) => (
              <button key={d.day} type="button" className={d.isToday ? "week-day-head today" : "week-day-head"} role="columnheader" onClick={() => goToDay(d.day)}>
                <span>{d.label}</span>
                <span className="mono o3">{formatHm(d.totalMs)}</span>
              </button>
            ))}
          </div>
          <div className="week-body" style={bodyStyle}>
            <TimeAxis bounds={bounds} pxPerMinute={pxPerMinute} />
            {week.days.map((d) => (
              <DayColumn key={d.day} column={d} pxPerMinute={pxPerMinute} focused={focused} />
            ))}
          </div>
        </div>
      )}
      {editor && (
        <Modal onClose={() => useStore.getState().closeEditor()} labelledBy={editorId} className="recording-editor-sheet">
          <RecordingEditor key={editor.mode === "edit" ? editor.id : "new"} id={editorId} />
        </Modal>
      )}
    </section>
  );
}
