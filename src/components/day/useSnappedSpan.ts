import { useEffect, useRef, useState } from "react";
import { api, type SnapEdge, type Span } from "../../api";

/** A span being dragged; `moving` is the end the pointer holds (none when it moves whole). */
export type DraggedSpan = Span & { moving?: SnapEdge };

/**
 * `span` on Rust's snap grid, asked at most once per animation frame while it changes. Until an answer arrives
 * the raw span stands in, for display only.
 */
export function useSnappedSpan(span: DraggedSpan | null): Span | null {
  const [snapped, setSnapped] = useState<{ raw: Span; snapped: Span } | null>(null);
  const frame = useRef(0);
  const start = span?.start;
  const end = span?.end;
  const moving = span?.moving ?? null;

  useEffect(() => {
    if (start === undefined || end === undefined) return;
    cancelAnimationFrame(frame.current);
    let alive = true;
    frame.current = requestAnimationFrame(() => {
      void api.recordingSnap(start, end, moving).then(
        (result) => alive && setSnapped({ raw: { start, end }, snapped: result }),
        () => {},
      );
    });
    return () => {
      alive = false;
      cancelAnimationFrame(frame.current);
    };
  }, [start, end, moving]);

  if (!span) return null;
  // An answer for an earlier span would show stale times.
  if (snapped && snapped.raw.start === span.start && snapped.raw.end === span.end) return snapped.snapped;
  return { start: Math.min(span.start, span.end), end: Math.max(span.start, span.end) };
}
