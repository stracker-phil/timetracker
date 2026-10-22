import type { Recording } from "../api";
import { formatClock } from "./format";

/** A saved recording's length: its closed spans. */
export const recordedMs = (r: Recording) => r.spans.reduce((sum, s) => sum + (s.end === null ? 0 : s.end - s.start), 0);

/** `14:25–15:00, 16:05–20:15`; an open span ends `now`. */
export const spansText = (spans: { start: number; end: number | null }[]) =>
  spans.map((s) => `${formatClock(s.start)}–${s.end === null ? "now" : formatClock(s.end)}`).join(", ");
