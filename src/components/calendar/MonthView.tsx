import type { CSSProperties } from "react";
import { api, type MonthDay } from "../../api";
import { formatHm } from "../../lib/format";
import { useApiResult } from "../../lib/useApiResult";
import { useStore } from "../../state/store";
import { shadeColor } from "../Accent";
import { CalendarHeader } from "./CalendarHeader";

const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function DayCell({ cell }: { cell: MonthDay }) {
  const classes = ["month-day", !cell.inMonth && "month-day--outside", cell.isToday && "month-day--today", cell.isWeekend && "month-day--weekend"];
  const label = `${cell.day}${cell.recordedMs > 0 ? `, ${formatHm(cell.recordedMs)} recorded` : ""}`;
  return (
    <button type="button" className={classes.filter(Boolean).join(" ")} aria-label={label} onClick={() => useStore.getState().goToDay(cell.day)}>
      <span className="month-day-head">
        <span className="month-day-number">{cell.dayOfMonth}</span>
        {cell.recordedMs > 0 && <span className="month-day-hours">{formatHm(cell.recordedMs)}</span>}
      </span>
      {cell.projects.length > 0 && (
        <span className="month-projects" aria-hidden="true">
          {cell.projects.map((p) => (
            <span key={p.projectId} className="month-project" title={`${p.name} · ${formatHm(p.ms)}`} style={{ "--rec": shadeColor(p.color, p.shade) } as CSSProperties}>
              {p.name}
            </span>
          ))}
        </span>
      )}
    </button>
  );
}

/** The month as a calendar: recorded hours per day and its main projects as filled bars; a click opens the day. */
export function MonthView() {
  const day = useStore((s) => s.day);
  const changes = useStore((s) => s.changeCount);
  const { data: month, error } = useApiResult(`month:${day}#${changes}`, () => api.monthView(day), { keepData: true });

  return (
    <section className="day-page" aria-label="Month">
      <CalendarHeader title={month?.label ?? ""}>
        {month && (
          <span className="day-totals o2" aria-label="Totals">
            recorded {formatHm(month.totalMs)}
          </span>
        )}
      </CalendarHeader>
      {error && !month && <p className="settings-alert">{error.message}</p>}
      {month && (
        <div className="month-grid" role="grid" aria-label={month.label}>
          {WEEKDAYS.map((w) => (
            <span key={w} className="month-weekday" role="columnheader">
              {w}
            </span>
          ))}
          {month.weeks.flat().map((cell) => (
            <DayCell key={cell.day} cell={cell} />
          ))}
        </div>
      )}
    </section>
  );
}
