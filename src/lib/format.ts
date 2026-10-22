/** `12,345`: one grouping style everywhere, whatever the system locale. */
export function formatCount(n: number): string {
  return n.toLocaleString("en-US");
}

/** `512 B`, `1.5 KB`, `12.3 MB` (binary units). */
export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${unit === 0 ? value : value.toFixed(1)} ${units[unit]}`;
}

/** The local calendar date of a UTC millisecond timestamp, `2026-10-21`. */
export function formatDay(ms: number): string {
  const date = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export const capitalize = (s: string) => s[0].toUpperCase() + s.slice(1);

export const plural = (n: number, one: string, many: string) => `${formatCount(n)} ${n === 1 ? one : many}`;

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;

/** A duration as `1 h 05 min`, `45 min` or `0 min`; seconds are dropped, negative counts as zero. */
export function formatDuration(ms: number): string {
  const minutes = Math.floor(Math.max(0, ms) / MINUTE_MS);
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours === 0) return `${rest} min`;
  return `${hours} h ${String(rest).padStart(2, "0")} min`;
}

/** Hours and minutes as `12:30`; negative counts as zero. */
export function formatHm(ms: number): string {
  const minutes = Math.floor(Math.max(0, ms) / MINUTE_MS);
  return `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, "0")}`;
}

/** Hours and minutes as `12:30 h`. */
export function formatHours(ms: number): string {
  return `${formatHm(ms)} h`;
}

/** A UTC millisecond timestamp as local wall-clock time, `09:30`. */
export function formatClock(ms: number): string {
  const date = new Date(ms);
  return `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}

const MINUTE = 60;
const HOUR = 3600;
const DAY = 86400;

/** "2 h ago" or "in 3 d" for an ISO date or a UTC millisecond timestamp. */
export function relativeTime(when: string | number, now = Date.now()): string {
  const ms = typeof when === "number" ? when : new Date(when).getTime();
  const diff = Math.round((ms - now) / 1000);
  const abs = Math.abs(diff);
  if (abs < 45) return "just now";
  let text: string;
  if (abs < HOUR) text = `${Math.round(abs / MINUTE)} min`;
  else if (abs < DAY) text = `${Math.round(abs / HOUR)} h`;
  else if (abs < DAY * 45) text = `${Math.round(abs / DAY)} d`;
  else if (abs < DAY * 365) text = `${Math.round(abs / (DAY * 30))} mo`;
  else text = `${(abs / (DAY * 365)).toFixed(1).replace(/\.0$/, "")} y`;
  return diff > 0 ? `in ${text}` : `${text} ago`;
}

export { HOUR_MS, MINUTE_MS };
