import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { api, type Range } from "../api";
import { addDays, addMonths, formatMonthLabel, today } from "../lib/day";
import { useApiResult } from "../lib/useApiResult";
import { useStore } from "../state/store";
import { useCalendarPeriod, useReportRange } from "./shownPeriod";
import { Modal } from "./Modal";
import { useAnchoredStyle } from "./useAnchoredStyle";

const WEEKDAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
const ARROW_STEPS: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };

/** The days to mark: the shown day, week or month in the calendar views, a custom range in Reports, else none. */
function useSelectedDays(): [string, string] | null {
  const view = useStore((s) => s.view);
  const range = useReportRange();
  const calendar = useCalendarPeriod();
  if (view === "reports") return customDays(range);
  return calendar && [calendar.period.from, calendar.period.to];
}

const customDays = (range: Range): [string, string] | null => (range.kind === "custom" ? [range.from, range.to] : null);

/** The day the calendar opens on: the shown day, or the first day of a custom report range. */
function startDay(): string {
  const s = useStore.getState();
  if (s.view !== "reports") return s.day;
  return customDays(s.reportFilter?.range ?? { kind: "today" })?.[0] ?? today();
}

/** A month calendar from Rust: a click or ↵ shows that day in the Day, Week or Month view, or its period in Reports. */
export function DatePickerPopover() {
  const close = useStore((s) => s.closeDialog);
  const showDate = useStore((s) => s.showDate);
  const style = useAnchoredStyle("[data-date-heading]", "left");
  const [focus, setFocus] = useState(startDay);
  const selected = useSelectedDays();
  const gridRef = useRef<HTMLDivElement>(null);
  const month = focus.slice(0, 7);
  const { data: grid } = useApiResult(`${month}:${selected?.join(":") ?? ""}`, () => api.calendarPicker(focus, selected), { keepData: true });

  // The focused day takes the keyboard focus on opening, and after arrow keys (which may turn the month).
  const opened = useRef(false);
  useEffect(() => {
    const el = gridRef.current;
    const target = el?.querySelector<HTMLElement>(`[data-day="${focus}"]`);
    if (!target) return;
    if (!opened.current || el?.contains(document.activeElement)) target.focus();
    opened.current = true;
  }, [focus, grid]);

  const pick = (day: string) => {
    close();
    showDate(day);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    const by = ARROW_STEPS[e.key];
    if (by !== undefined) setFocus(addDays(focus, by));
    else if (e.key === "PageUp" || e.key === "PageDown") setFocus(addMonths(focus, e.key === "PageUp" ? -1 : 1));
    else return;
    e.preventDefault();
  };

  return (
    <Modal onClose={close} variant="popover" className="date-picker" label="Pick a date" style={style}>
      <div className="date-picker-head">
        <button type="button" className="icon-btn" aria-label="Previous month" onClick={() => setFocus(addMonths(focus, -1))}>
          <ChevronLeft size="1.077rem" />
        </button>
        <span className="date-picker-month" aria-live="polite">
          {formatMonthLabel(focus)}
        </span>
        <button type="button" className="icon-btn" aria-label="Next month" onClick={() => setFocus(addMonths(focus, 1))}>
          <ChevronRight size="1.077rem" />
        </button>
      </div>
      <div className="date-picker-grid" role="grid" aria-label={formatMonthLabel(focus)} ref={gridRef} onKeyDown={onKeyDown}>
        {WEEKDAYS.map((w) => (
          <span key={w} className="date-picker-weekday" role="columnheader">
            {w}
          </span>
        ))}
        {grid?.weeks.flat().map((cell) => {
          const classes = ["date-picker-day", cell.outside && "outside", cell.isToday && "today", cell.selected && "sel"];
          return (
            <button
              key={cell.day}
              type="button"
              role="gridcell"
              className={classes.filter(Boolean).join(" ")}
              aria-label={cell.day}
              aria-selected={cell.selected}
              aria-current={cell.isToday ? "date" : undefined}
              tabIndex={cell.day === focus ? 0 : -1}
              data-day={cell.day}
              onClick={() => pick(cell.day)}
            >
              {Number(cell.day.slice(8))}
            </button>
          );
        })}
      </div>
    </Modal>
  );
}
