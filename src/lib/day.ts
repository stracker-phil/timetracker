import { formatDay } from "./format";

/** Labels of `YYYY-MM-DD` days; stepping, bounds and durations stay in Rust (ADR 0011). */

function parts(day: string): [number, number, number] {
  const [y, m, d] = day.split("-").map(Number);
  return [y, m - 1, d];
}

export function today(): string {
  return formatDay(Date.now());
}

export function isToday(day: string): boolean {
  return day === today();
}

/** `Tue, Oct 21`, with the year when it is not this year's. */
export function formatDayLabel(day: string): string {
  const [y, m, d] = parts(day);
  const date = new Date(y, m, d);
  const sameYear = y === new Date().getFullYear();
  return date.toLocaleDateString("en-US", { weekday: "short", month: "short", day: "numeric", ...(sameYear ? {} : { year: "numeric" }) });
}

const monthDay = (y: number, m: number, d: number) => new Date(y, m, d).toLocaleDateString("en-US", { month: "short", day: "numeric" });

/** `Oct 19 – 25`, `Oct 26 – Nov 1`, `Dec 29, 2025 – Jan 4, 2026`; the year only when it is not this year's. */
export function formatRangeLabel(from: string, to: string): string {
  if (from === to) return formatDayLabel(from);
  const [fy, fm, fd] = parts(from);
  const [ty, tm, td] = parts(to);
  if (fy !== ty) return `${monthDay(fy, fm, fd)}, ${fy} – ${monthDay(ty, tm, td)}, ${ty}`;
  const year = fy === new Date().getFullYear() ? "" : `, ${fy}`;
  const end = fm === tm ? String(td) : monthDay(ty, tm, td);
  return `${monthDay(fy, fm, fd)} – ${end}${year}`;
}

/** `October 2026`. */
export function formatMonthLabel(day: string): string {
  const [y, m] = parts(day);
  return new Date(y, m, 1).toLocaleDateString("en-US", { month: "long", year: "numeric" });
}
