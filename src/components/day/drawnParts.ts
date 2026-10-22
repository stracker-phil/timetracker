import type { Span } from "../../api";

/** The parts of `spans` inside the shown day, for drawing only (nothing is saved from them). */
export function drawnParts(spans: Span[], [first, last]: [number, number]): Span[] {
  return spans.filter((s) => s.end > first && s.start < last).map((s) => ({ start: Math.max(s.start, first), end: Math.min(s.end, last) }));
}

/** The snapped selection's own band, unless it is already drawn as one of `draftBands`. */
export function selectionBand(selection: Span | null, draftBands: Span[]): Span | null {
  return selection && !draftBands.some((b) => b.start === selection.start && b.end === selection.end) ? selection : null;
}
