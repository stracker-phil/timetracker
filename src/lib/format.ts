/** `12,345`: one grouping style everywhere, whatever the system locale. */
export function formatCount(n: number): string {
  return n.toLocaleString("en-US");
}

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
