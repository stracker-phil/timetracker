import { formatDay } from "./format";

/**
 * Today and the month label of `YYYY-MM-DD` days. Day and range labels, stepping, bounds and durations stay in
 * Rust (ADR 0011), which reads the locale's date order.
 */

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

/** `October 2026`: the same in every date order. */
export function formatMonthLabel(day: string): string {
  const [y, m] = parts(day);
  return new Date(y, m, 1).toLocaleDateString("en-US", { month: "long", year: "numeric" });
}
