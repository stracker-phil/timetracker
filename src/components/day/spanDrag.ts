import type { Span } from "../../api";
import { tickNear, type TickAt, type TickGrid } from "./tickPointer";

/** What a press holds: the whole span, or one of its edges. */
export type Grip = "move" | "start" | "end";

/**
 * The span a selection drag covers, from the tick it was pressed on (`origin`, fixed for the drag) to the
 * pointer's tick; on the same tick, the one tick on the pointer's side, so it never has zero length.
 */
export function selectedSpan(ticks: number[], origin: number, at: TickAt): Span {
  let first = Math.min(origin, at.index);
  let last = Math.max(origin, at.index);
  if (first === last) {
    if ((at.above && first > 0) || last === ticks.length - 1) first--;
    else last++;
  }
  return { start: ticks[first], end: ticks[last] };
}

/** The first tick after `ms`. */
const tickAfter = (ticks: number[], ms: number) => ticks.find((t) => t > ms);

/** The last tick before `ms`. */
function tickBefore(ticks: number[], ms: number): number | undefined {
  for (let i = ticks.length - 1; i >= 0; i--) if (ticks[i] < ms) return ticks[i];
  return undefined;
}

/**
 * `span` with its `edge` on the tick at `index`, kept a tick past the other edge, which keeps its time (also when
 * it is off the grid).
 */
export function spanWithEdge(ticks: number[], span: Span, edge: "start" | "end", index: number): Span {
  const tick = ticks[index];
  if (edge === "end") {
    const end = tick > span.start ? tick : tickAfter(ticks, span.start);
    return end === undefined ? span : { start: span.start, end };
  }
  const start = tick < span.end ? tick : tickBefore(ticks, span.end);
  return start === undefined ? span : { start, end: span.end };
}

/**
 * `span` moved whole by `by` ticks, both edges on ticks, kept inside the grid; unchanged when it does not move.
 * An edge off the grid starts from the tick drawn nearest it.
 */
export function spanMoved(grid: TickGrid, span: Span, by: number): Span {
  const { ticks } = grid;
  const first = tickNear(grid, span.start);
  const last = Math.max(tickNear(grid, span.end), Math.min(first + 1, ticks.length - 1));
  const shift = Math.max(-first, Math.min(by, ticks.length - 1 - last));
  return shift === 0 ? span : { start: ticks[first + shift], end: ticks[last + shift] };
}

export const sameSpan = (a: Span, b: Span) => a.start === b.start && a.end === b.end;
