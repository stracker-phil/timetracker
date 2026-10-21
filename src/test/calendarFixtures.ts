// Must not import the store or "../api" at runtime: apiMock.ts imports this file while the api module is mocked.
import type { DayColumn, MonthDay, MonthView, WeekView } from "../api/types";
import { dayBounds, DAY } from "./dayFixtures";

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

/** One day of a week; `patch` overrides fields. `label` defaults to `Tue 10`. */
export function dayColumn(patch: Partial<DayColumn> = {}): DayColumn {
  const day = patch.day ?? DAY;
  const date = parse(day);
  return {
    day,
    bounds: dayBounds(day),
    label: `${WEEKDAYS[(date.getDay() + 6) % 7]} ${date.getDate()}`,
    isToday: false,
    recordings: [],
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

/** A calendar cell without recordings; `patch` overrides fields. */
export function monthDay(patch: Partial<MonthDay> = {}): MonthDay {
  return { day: DAY, inMonth: true, isToday: false, recordedMs: 0, projects: [], ...patch };
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
