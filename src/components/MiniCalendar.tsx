import { ChevronLeft, ChevronRight } from "lucide-react";
import { useState } from "react";
import { api, type MiniDay, type MiniWeek } from "../api";
import { today } from "../lib/day";
import { useApiResult } from "../lib/useApiResult";
import { useStore } from "../state/store";
import { useShown, useShownDay } from "./shownPeriod";

const WEEKDAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

function show(view: "week" | "month", day: string) {
  const { setView, showDate } = useStore.getState();
  setView(view);
  showDate(day);
}

function DayCell({ cell }: { cell: MiniDay }) {
  const classes = ["mini-day", !cell.inMonth && "out", cell.isToday && "today", cell.selected && "sel", cell.hasRecordings && "recorded"];
  return (
    <button
      type="button"
      tabIndex={-1}
      className={classes.filter(Boolean).join(" ")}
      aria-label={cell.day}
      aria-current={cell.selected ? "date" : undefined}
      onClick={() => useStore.getState().goToDay(cell.day)}
    >
      <span className="mini-day-number">{cell.dayOfMonth}</span>
    </button>
  );
}

function WeekRow({ week }: { week: MiniWeek }) {
  return (
    <div className={week.selected ? "mini-week sel" : "mini-week"}>
      <button
        type="button"
        tabIndex={-1}
        className="mini-week-number"
        aria-label={`Week ${week.number}`}
        aria-current={week.selected ? "true" : undefined}
        onClick={() => show("week", week.start)}
      >
        {week.number}
      </button>
      {week.days.map((cell) => (
        <DayCell key={cell.day} cell={cell} />
      ))}
    </div>
  );
}

/**
 * The month of the shown date, Monday first with week numbers; the shown day, week or month is highlighted. A
 * day, a week number or the title shows that day, week or month; ‹ › page the calendar only.
 */
export function MiniCalendar() {
  const shownDay = useShownDay();
  const shown = useShown();
  const changes = useStore((s) => s.changeCount);
  // Months paged with ‹ ›, from the shown day's; a new shown day starts over.
  const [paged, setPaged] = useState({ day: shownDay, offset: 0 });
  const offset = paged.day === shownDay ? paged.offset : 0;
  const page = (by: number) => setPaged({ day: shownDay, offset: offset + by });
  const key = `${shownDay}|${offset}|${JSON.stringify(shown)}|${today()}|${changes}`;
  const { data } = useApiResult(key, () => api.calendarMiniMonth(shownDay, offset, shown), { keepData: true });

  return (
    <div className="mini-calendar" role="group" aria-label="Calendar">
      <div className="mini-head">
        <button
          type="button"
          className={data?.monthSelected ? "mini-title sel" : "mini-title"}
          aria-current={data?.monthSelected ? "true" : undefined}
          data-date-heading
          onClick={() => data && show("month", data.monthStart)}
        >
          {data && (
            <>
              {data.monthName} <span className="mini-year">{data.year}</span>
            </>
          )}
        </button>
        <button type="button" className="icon-btn" aria-label="Previous month" onClick={() => page(-1)}>
          <ChevronLeft size="1.077rem" />
        </button>
        <button type="button" className="icon-btn" aria-label="Next month" onClick={() => page(1)}>
          <ChevronRight size="1.077rem" />
        </button>
      </div>
      <div className="mini-grid">
        <div className="mini-week mini-weekdays" aria-hidden="true">
          <span className="mini-week-number">Wk</span>
          {WEEKDAYS.map((w) => (
            <span key={w} className="mini-weekday">
              {w}
            </span>
          ))}
        </div>
        {data?.weeks.map((week) => (
          <WeekRow key={week.start} week={week} />
        ))}
      </div>
    </div>
  );
}
