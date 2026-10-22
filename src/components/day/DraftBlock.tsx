import { useState, type PointerEvent as ReactPointerEvent } from "react";
import type { Span } from "../../api";
import { formatClock } from "../../lib/format";
import { useStore } from "../../state/store";
import { drawnParts } from "./drawnParts";
import { pointerDrag } from "./pointerDrag";
import { spanAfterDrag } from "./spanDrag";
import { useSnappedSpan, type DraggedSpan } from "./useSnappedSpan";

interface Props {
  /** The open editor's span, whole (it may reach into another day). */
  span: Span;
  /** Its place in the editor's spans. */
  index: number;
  y: (ms: number) => number;
  /** The shown day's first and last instant: the span is drawn cut to them. */
  bounds: [number, number];
  pxPerMinute: number;
}

/**
 * One of the open editor's spans in the time column, with its times; drag its top or bottom edge to move that end
 * on the snap grid. On release Rust merges it with the spans it then touches or overlaps.
 */
export function DraftBlock({ span, index, y, bounds, pxPerMinute }: Props) {
  const [drag, setDrag] = useState<DraggedSpan | null>(null);
  const shown = useSnappedSpan(drag) ?? span;
  const [part] = drawnParts([shown], bounds);
  if (!part) return null;
  // An edge in another day does not move here.
  const movable = { start: span.start >= bounds[0], end: span.end <= bounds[1] };

  const press = (edge: "start" | "end") => (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    const at = (dy: number) => spanAfterDrag(span, edge, dy, pxPerMinute);
    pointerDrag(e, {
      move: (dy) => setDrag(at(dy)),
      end: (dy, dragged) => {
        if (!dragged) return setDrag(null);
        // The snapped ghost stays until Rust's answer replaces the span.
        const { start, end } = at(dy);
        void useStore
          .getState()
          .resizeDraftSpan(index, { start, end }, edge)
          .finally(() => setDrag(null));
      },
    });
  };

  return (
    <div
      className={drag ? "recording-draft dragging" : "recording-draft"}
      style={{ top: y(part.start), height: Math.max(2, y(part.end) - y(part.start)) }}
      aria-label={`Draft ${formatClock(part.start)}–${formatClock(part.end)}`}
    >
      <span className="recording-draft-time">{formatClock(part.start)}</span>
      <span className="recording-draft-time">{formatClock(part.end)}</span>
      {movable.start && <span className="grip start" aria-label="Drag to change the start" onPointerDown={press("start")} />}
      {movable.end && <span className="grip end" aria-label="Drag to change the end" onPointerDown={press("end")} />}
    </div>
  );
}
