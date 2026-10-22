import type { Span } from "../../api";
import type { DraggedSpan } from "./useSnappedSpan";

const MINUTE_MS = 60_000;

/** What a press holds: the whole span, or one of its edges. */
export type Grip = "move" | "start" | "end";

/** `px` on a timeline drawn at `pxPerMinute`, in whole milliseconds: Rust takes no fractions. */
export const msOfPx = (px: number, pxPerMinute: number) => Math.round((px / pxPerMinute) * MINUTE_MS);

/**
 * Where the pointer has dragged `grip` of `span` to after `dy` px, unsnapped: Rust snaps it, keeping a dragged
 * end a step from the other.
 */
export function spanAfterDrag(span: Span, grip: Grip, dy: number, pxPerMinute: number): DraggedSpan {
  const shift = msOfPx(dy, pxPerMinute);
  const start = grip === "end" ? span.start : span.start + shift;
  const end = grip === "start" ? span.end : span.end + shift;
  return grip === "move" ? { start, end } : { start, end, moving: grip };
}
