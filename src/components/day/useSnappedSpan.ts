import { useEffect, useRef, useState } from "react";
import { api, type Span } from "../../api";

/**
 * `span` as Rust will store it, asked at most once per animation frame while it changes. Until an answer
 * arrives the raw span stands in, for display only.
 */
export function useSnappedSpan(span: Span | null): Span | null {
  const [snapped, setSnapped] = useState<{ raw: Span; snapped: Span } | null>(null);
  const frame = useRef(0);
  const start = span?.start;
  const end = span?.end;

  useEffect(() => {
    if (start === undefined || end === undefined) return;
    cancelAnimationFrame(frame.current);
    let alive = true;
    frame.current = requestAnimationFrame(() => {
      void api.recordingSnap(Math.min(start, end), Math.max(start, end)).then(
        (result) => alive && setSnapped({ raw: { start, end }, snapped: result }),
        () => {},
      );
    });
    return () => {
      alive = false;
      cancelAnimationFrame(frame.current);
    };
  }, [start, end]);

  if (!span) return null;
  // An answer for an earlier span would show (and save) stale times.
  if (snapped && snapped.raw.start === span.start && snapped.raw.end === span.end) return snapped.snapped;
  return { start: Math.min(span.start, span.end), end: Math.max(span.start, span.end) };
}
