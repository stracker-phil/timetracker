import type { PointerEvent as ReactPointerEvent } from "react";
import type { Span } from "../../api";
import { formatClock } from "../../lib/format";
import { useStore } from "../../state/store";
import { drawnParts } from "./drawnParts";
import { pointerDrag } from "./pointerDrag";
import { sameSpan, spanWithEdge } from "./spanDrag";
import { tickPointer, type TickGrid } from "./tickPointer";

interface Props {
  /** The open editor's span, whole (it may reach into another day). */
  span: Span;
  /** Its place in the editor's spans. */
  index: number;
  grid: TickGrid;
  /** The shown day's first and last instant: the span is drawn cut to them. */
  bounds: [number, number];
}

/**
 * One of the open editor's spans in the time column, with its times; drag its top or bottom edge to move that end
 * from tick to tick while the other keeps its time. On release Rust merges it with the spans it then touches or
 * overlaps.
 */
export function DraftBlock({ span, index, grid, bounds }: Props) {
  const dragged = useStore((s) => (s.draftDrag?.index === index ? s.draftDrag.span : null));
  const shown = dragged ?? span;
  const [part] = drawnParts([shown], bounds);
  if (!part) return null;
  // An edge in another day does not move here.
  const onGrid = grid.ticks.length > 1;
  const movable = { start: onGrid && span.start >= bounds[0], end: onGrid && span.end <= bounds[1] };

  const press = (edge: "start" | "end") => (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    const tickAt = tickPointer(grid, e.currentTarget);
    let last = span;
    const { setDraftDrag, resizeDraftSpan } = useStore.getState();
    pointerDrag(e, {
      move: (_dy, ev) => {
        last = spanWithEdge(grid.ticks, span, edge, tickAt(ev.clientY).index);
        setDraftDrag({ index, span: last });
      },
      // The dragged span stays until Rust's answer replaces it.
      end: () => void (sameSpan(last, span) ? setDraftDrag(null) : resizeDraftSpan(index, last, edge)),
    });
  };

  return (
    <div
      className={dragged ? "recording-draft dragging" : "recording-draft"}
      style={{ top: grid.y(part.start), height: Math.max(2, grid.y(part.end) - grid.y(part.start)) }}
      aria-label={`Draft ${formatClock(part.start)}–${formatClock(part.end)}`}
    >
      <span className="recording-draft-time">{formatClock(part.start)}</span>
      <span className="recording-draft-time">{formatClock(part.end)}</span>
      {movable.start && <span className="grip start" aria-label="Drag to change the start" onPointerDown={press("start")} />}
      {movable.end && <span className="grip end" aria-label="Drag to change the end" onPointerDown={press("end")} />}
    </div>
  );
}
