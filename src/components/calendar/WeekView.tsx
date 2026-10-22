import { useState } from "react";
import { Plus } from "lucide-react";
import { api, type DayColumn as Column } from "../../api";
import { formatHm } from "../../lib/format";
import { useApiResult } from "../../lib/useApiResult";
import { useStore } from "../../state/store";
import { Toolbar } from "../Toolbar";
import { EditorSheet } from "./EditorSheet";
import { WeekCard, type CardDrag } from "./WeekCard";

/** `Mon 20`. */
const columnName = (column: Column) => `${column.weekday} ${column.dayOfMonth}`;

function DayHead({ column }: { column: Column }) {
  const classes = ["week-day-head", column.isToday && "today", column.isWeekend && "weekend"];
  return (
    <button
      type="button"
      className={classes.filter(Boolean).join(" ")}
      role="columnheader"
      aria-label={`${columnName(column)}, ${formatHm(column.totalMs)} recorded`}
      onClick={() => useStore.getState().goToDay(column.day)}
    >
      <span className="week-day-name">{column.isToday ? "Today" : columnName(column)}</span>
      <span className={column.totalMs > 0 ? "week-day-total" : "week-day-total o3"}>{formatHm(column.totalMs)}</span>
    </button>
  );
}

interface ColumnProps {
  column: Column;
  focused: string | null;
  drag: CardDrag | null;
  onDrag: (drag: CardDrag | null) => void;
}

function DayColumn({ column, focused, drag, onDrag }: ColumnProps) {
  const dropping = drag !== null && drag.over === column.day && drag.from !== column.day;
  const classes = ["week-column", column.isToday && "week-column--today", column.isWeekend && "week-column--weekend", dropping && "week-column--drop"];
  return (
    <div className={classes.filter(Boolean).join(" ")} data-day={column.day} aria-label={columnName(column)}>
      {column.entries.map((e) => (
        <WeekCard key={e.id} entry={e} day={column.day} focused={e.id === focused} dragging={drag?.id === e.id && drag.from === column.day} onDrag={onDrag} />
      ))}
      <button type="button" className={column.entries.length > 0 ? "week-add" : "week-add week-add--empty"} aria-label={`New recording on ${columnName(column)}`} title="New recording" onClick={() => void useStore.getState().openNewRecording(column.day)}>
        <Plus size="1rem" aria-hidden="true" />
        <span>New recording</span>
      </button>
    </div>
  );
}

/** Seven columns of the days' recordings, Monday first, without a time axis; drag a card to another day to move it, with ⌥ to copy it. */
export function WeekView() {
  const day = useStore((s) => s.day);
  const changes = useStore((s) => s.changeCount);
  const focused = useStore((s) => s.focusedRecording);
  const [drag, setDrag] = useState<CardDrag | null>(null);
  const { data: week, error } = useApiResult(`week:${day}#${changes}`, () => api.weekView(day), { keepData: true });

  return (
    <section className="day-page" aria-label="Week">
      <Toolbar
        title={week?.label ?? ""}
        detail={
          week && (
            <span className="toolbar-detail" aria-label="Totals">
              recorded {formatHm(week.totalMs)} · tracked {formatHm(week.trackedMs)}
            </span>
          )
        }
      />
      {error && !week && <p className="settings-alert">{error.message}</p>}
      {week && (
        <div className="week">
          <div className="week-head" role="row">
            {week.days.map((d) => (
              <DayHead key={d.day} column={d} />
            ))}
          </div>
          <div className="week-body">
            {week.days.map((d) => (
              <DayColumn key={d.day} column={d} focused={focused} drag={drag} onDrag={setDrag} />
            ))}
          </div>
        </div>
      )}
      <EditorSheet />
    </section>
  );
}
