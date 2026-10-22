import type { CSSProperties } from "react";
import { Plus } from "lucide-react";
import { api, type CalendarEntry, type MonthDay } from "../../api";
import { formatDuration } from "../../lib/format";
import { useApiResult } from "../../lib/useApiResult";
import { useStore } from "../../state/store";
import { shadeColor } from "../Accent";
import { Toolbar } from "../Toolbar";
import { EditorSheet } from "./EditorSheet";
import { editEntry, entryName } from "./entry";

/** One line per recording, the same height whatever its duration; a click edits it. */
function EntryLine({ entry }: { entry: CalendarEntry }) {
  return (
    <button
      type="button"
      className="month-entry"
      style={{ "--rec": shadeColor(entry.color, entry.shade) } as CSSProperties}
      title={`${entryName(entry)} · ${formatDuration(entry.durationMs)}`}
      aria-label={`${entryName(entry)}, ${formatDuration(entry.durationMs)}`}
      onClick={() => editEntry(entry)}
    >
      <span className="month-entry-dot" aria-hidden="true" />
      <span className="month-entry-name">{entry.note || entry.projectName}</span>
      <span className="month-entry-time">{formatDuration(entry.durationMs)}</span>
    </button>
  );
}

function DayCell({ cell }: { cell: MonthDay }) {
  const classes = ["month-day", !cell.inMonth && "month-day--outside", cell.isToday && "month-day--today", cell.isWeekend && "month-day--weekend"];
  const name = `${cell.weekday} ${cell.dayOfMonth}`;
  return (
    <div className={classes.filter(Boolean).join(" ")} role="gridcell" aria-label={cell.day}>
      <div className="month-day-head">
        <button type="button" className="month-day-date" aria-label={`${cell.day}${cell.recordedMs > 0 ? `, ${formatDuration(cell.recordedMs)} recorded` : ""}`} onClick={() => useStore.getState().goToDay(cell.day)}>
          <span className="month-day-weekday">{cell.weekday}</span>
          <span className="month-day-number">{cell.dayOfMonth}</span>
        </button>
        <button type="button" className="month-day-add" aria-label={`New recording on ${name}`} title="New recording" onClick={() => void useStore.getState().openNewRecording(cell.day)}>
          <Plus size="0.923rem" aria-hidden="true" />
        </button>
        {cell.recordedMs > 0 && <span className="month-day-hours">{formatDuration(cell.recordedMs)}</span>}
      </div>
      {cell.entries.map((e) => (
        <EntryLine key={e.id} entry={e} />
      ))}
    </div>
  );
}

/** The month as rows of whole weeks, Monday first: each day lists its recordings, one line each; a busy day makes its week taller. */
export function MonthView() {
  const day = useStore((s) => s.day);
  const changes = useStore((s) => s.changeCount);
  const { data: month, error } = useApiResult(`month:${day}#${changes}`, () => api.monthView(day), { keepData: true });

  return (
    <section className="day-page" aria-label="Month">
      <Toolbar
        title={month?.label ?? ""}
        detail={
          month && (
            <span className="toolbar-detail" aria-label="Totals">
              recorded {formatDuration(month.totalMs)}
            </span>
          )
        }
      />
      {error && !month && <p className="settings-alert">{error.message}</p>}
      {month && (
        <div className="month-grid" role="grid" aria-label={month.label}>
          {month.weeks.map((week) => (
            <div key={week[0].day} className="month-week" role="row">
              {week.map((cell) => (
                <DayCell key={cell.day} cell={cell} />
              ))}
            </div>
          ))}
        </div>
      )}
      <EditorSheet />
    </section>
  );
}
