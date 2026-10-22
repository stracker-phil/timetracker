// Must not import the store or "../api" at runtime: apiMock.ts imports this file while the api module is mocked.
import type { CalendarEntry, DayColumn, MiniDay, MiniMonth, MiniWeek, MonthDay, MonthView, WeekView } from "../api/types";
import { at, DAY } from "./dayFixtures";

/** A 45-minute recording of "Website" at 09:00 on `DAY`, as the Week and Month list it; `patch` overrides fields. */
export function calendarEntry(patch: Partial<CalendarEntry> = {}): CalendarEntry {
  const id = patch.id ?? "r1";
  return {
    heightUnits: 3,
    showNote: true,
    id,
    projectId: "p-site",
    projectName: "Website",
    clientName: null,
    color: "blue",
    shade: 0,
    note: "",
    tag: null,
    tagEmoji: null,
    durationMs: 45 * 60_000,
    start: at("09:00"),
    end: at("09:45"),
    spanIds: [`${id}-s1`],
    running: false,
    ...patch,
  };
}

const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

const parse = (day: string): Date => {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y, m - 1, d);
};
const format = (date: Date): string => `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
const shift = (day: string, by: number): string => {
  const date = parse(day);
  return format(new Date(date.getFullYear(), date.getMonth(), date.getDate() + by));
};

/** The Monday of the week containing `day`. */
export function mondayOf(day: string): string {
  return shift(day, -((parse(day).getDay() + 6) % 7));
}

const isoWeek = (monday: string): number => {
  const thursday = parse(shift(monday, 3));
  const jan1 = new Date(thursday.getFullYear(), 0, 1);
  return Math.floor(Math.round((thursday.getTime() - jan1.getTime()) / 86_400_000) / 7) + 1;
};

/**
 * The sidebar calendar for the month of `monthStart` (default the month of `DAY`), Monday first. `selected` is the
 * highlighted day or `[from, to]` range; `today` and `recorded` mark days. The title is highlighted by `monthSelected`.
 */
export function miniMonth(patch: Partial<MiniMonth> & { selected?: string | [string, string]; today?: string; recorded?: string[] } = {}): MiniMonth {
  const { selected, today, recorded = [], ...rest } = patch;
  const monthStart = patch.monthStart ?? `${DAY.slice(0, 7)}-01`;
  const first = parse(monthStart);
  const last = format(new Date(first.getFullYear(), first.getMonth() + 1, 0));
  const gridStart = mondayOf(monthStart);
  const [from, to] = Array.isArray(selected) ? selected : [selected, selected];
  const inRange = (day: string) => from !== undefined && to !== undefined && day >= from && day <= to;
  const count = Math.ceil(((parse(last).getTime() - parse(gridStart).getTime()) / 86_400_000 + 1) / 7);
  const weeks = Array.from({ length: count }, (_, w): MiniWeek => {
    const start = shift(gridStart, w * 7);
    const days = Array.from({ length: 7 }, (_, i): MiniDay => {
      const day = shift(start, i);
      return { day, dayOfMonth: parse(day).getDate(), inMonth: day.slice(0, 7) === monthStart.slice(0, 7), isToday: day === today, hasRecordings: recorded.includes(day), selected: inRange(day) };
    });
    return { number: isoWeek(start), start, selected: days.some((d) => d.selected), days };
  });
  return { monthStart, monthName: MONTHS[first.getMonth()], year: first.getFullYear(), monthSelected: false, weeks, ...rest };
}

/** One day of a week; `patch` overrides fields. `weekday`, `dayOfMonth` and `isWeekend` default from the day. */
export function dayColumn(patch: Partial<DayColumn> = {}): DayColumn {
  const day = patch.day ?? DAY;
  const date = parse(day);
  const weekday = WEEKDAYS[(date.getDay() + 6) % 7];
  return {
    day,
    weekday,
    dayOfMonth: date.getDate(),
    isToday: false,
    isWeekend: date.getDay() === 0 || date.getDay() === 6,
    entries: [],
    totalMs: 0,
    trackedMs: 0,
    ...patch,
  };
}

/**
 * The week of `weekStart` (a Monday, default the week of `DAY`) with empty days; `today` marks one of them.
 * `days` replaces the columns; the totals default to the sum of the days.
 */
export function weekView(patch: Partial<WeekView> & { today?: string } = {}): WeekView {
  const { today, ...rest } = patch;
  const weekStart = patch.weekStart ?? mondayOf(DAY);
  const days = patch.days ?? Array.from({ length: 7 }, (_, i) => dayColumn({ day: shift(weekStart, i), isToday: shift(weekStart, i) === today }));
  return {
    weekStart,
    label: `Week of ${weekStart}`,
    days,
    totalMs: days.reduce((sum, d) => sum + d.totalMs, 0),
    trackedMs: days.reduce((sum, d) => sum + d.trackedMs, 0),
    ...rest,
  };
}

/** A calendar cell without recordings; `patch` overrides fields. `weekday` defaults from the day. */
export function monthDay(patch: Partial<MonthDay> = {}): MonthDay {
  const day = patch.day ?? DAY;
  const date = parse(day);
  const weekday = WEEKDAYS[(date.getDay() + 6) % 7];
  return { day, weekday, dayOfMonth: date.getDate(), inMonth: true, isToday: false, isWeekend: date.getDay() === 0 || date.getDay() === 6, recordedMs: 0, entries: [], ...patch };
}

/**
 * The month of `monthStart` (default the month of `DAY`) as Monday-to-Sunday rows of empty cells; `today` marks one.
 * `cells` patches cells by day. The total defaults to the sum of the month's own days.
 */
export function monthView(patch: Partial<MonthView> & { today?: string; cells?: Record<string, Partial<MonthDay>> } = {}): MonthView {
  const { today, cells = {}, ...rest } = patch;
  const monthStart = patch.monthStart ?? `${DAY.slice(0, 7)}-01`;
  const first = parse(monthStart);
  const last = new Date(first.getFullYear(), first.getMonth() + 1, 0);
  const gridStart = mondayOf(monthStart);
  const count = Math.ceil(((parse(format(last)).getTime() - parse(gridStart).getTime()) / 86_400_000 + 1) / 7) * 7;
  const all = Array.from({ length: count }, (_, i) => {
    const day = shift(gridStart, i);
    return monthDay({ day, inMonth: day.slice(0, 7) === monthStart.slice(0, 7), isToday: day === today, ...cells[day] });
  });
  const weeks = Array.from({ length: count / 7 }, (_, i) => all.slice(i * 7, i * 7 + 7));
  return {
    monthStart,
    label: `${MONTHS[first.getMonth()]} ${first.getFullYear()}`,
    weeks,
    totalMs: all.filter((d) => d.inMonth).reduce((sum, d) => sum + d.recordedMs, 0),
    ...rest,
  };
}
