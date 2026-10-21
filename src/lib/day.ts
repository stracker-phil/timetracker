import { formatDay } from "./format";

/** Calendar arithmetic on `YYYY-MM-DD` days; bounds and durations stay in Rust (ADR 0011). */

function parts(day: string): [number, number, number] {
  const [y, m, d] = day.split("-").map(Number);
  return [y, m - 1, d];
}

/** The day `by` calendar days after `day` (before, when negative). */
export function addDays(day: string, by: number): string {
  const [y, m, d] = parts(day);
  return formatDay(new Date(y, m, d + by).getTime());
}

/** The first day of the month `by` months from `day`'s. */
export function addMonths(day: string, by: number): string {
  const [y, m] = parts(day);
  const first = new Date(y, m + by, 1);
  return `${first.getFullYear()}-${String(first.getMonth() + 1).padStart(2, "0")}-01`;
}

export function today(): string {
  return formatDay(Date.now());
}

export function isToday(day: string): boolean {
  return day === today();
}

/** Whole calendar days from `from` to `to`; negative when `to` is earlier. */
export function daysBetween(from: string, to: string): number {
  const [fy, fm, fd] = parts(from);
  const [ty, tm, td] = parts(to);
  return Math.round((Date.UTC(ty, tm, td) - Date.UTC(fy, fm, fd)) / 86_400_000);
}

/** `Tue, Oct 21`, with the year when it is not this year's. */
export function formatDayLabel(day: string): string {
  const [y, m, d] = parts(day);
  const date = new Date(y, m, d);
  const sameYear = y === new Date().getFullYear();
  return date.toLocaleDateString("en-US", { weekday: "short", month: "short", day: "numeric", ...(sameYear ? {} : { year: "numeric" }) });
}
