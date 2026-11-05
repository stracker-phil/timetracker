import type { PointerEvent as ReactPointerEvent } from "react";
import type { Span } from "../../api";
import { formatClock } from "../../lib/format";
import { useStore } from "../../state/store";
import { drawnParts } from "./drawnParts";
import { pointerDrag } from "./pointerDrag";
import { sameSpan, spanMoved, spanWithEdge, type Grip } from "./spanDrag";
import { tickPointer, type TickGrid } from "./tickPointer";
import { useDraftColor } from "./useDraftColor";

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
 * One of a new recording's (or suggestion's) spans in the time column, with its times; drag its body to move it,
 * its top or bottom edge to move that end while the other keeps its time, both from tick to tick. On release Rust
 * merges it with the spans it then touches or overlaps.
 */
export function DraftBlock({ span, index, grid, bounds }: Props) {
  const dragged = useStore((s) => (s.draftDrag?.index === index ? s.draftDrag.span : null));
  const color = useDraftColor();
  const shown = dragged ?? span;
  const [part] = drawnParts([shown], bounds);
  if (!part) return null;
  // An edge in another day does not move here.
  const onGrid = grid.ticks.length > 1;
  const movable = { start: onGrid && span.start >= bounds[0], end: onGrid && span.end <= bounds[1] };

  // A press on the body is the draft's, never a selection on the column below.
  const press = (grip: Grip) => (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    if (grip === "move" && !(movable.start && movable.end)) return;
    const tickAt = tickPointer(grid, e.currentTarget);
    const origin = tickAt(e.clientY).index;
    let last = span;
    const { setDraftDrag, resizeDraftSpan } = useStore.getState();
    pointerDrag(e, {
      move: (_dy, ev) => {
        const at = tickAt(ev.clientY).index;
        last = grip === "move" ? spanMoved(grid, span, at - origin) : spanWithEdge(grid.ticks, span, grip, at);
        setDraftDrag({ index, span: last });
      },
      // The dragged span stays until Rust's answer replaces it.
      end: () => void (sameSpan(last, span) ? setDraftDrag(null) : resizeDraftSpan(index, last, grip === "move" ? null : grip)),
    });
  };

  return (
    <div
      className={["recording-draft", color && "has-project", dragged && "dragging"].filter(Boolean).join(" ")}
      style={{ top: grid.y(part.start), height: Math.max(2, grid.y(part.end) - grid.y(part.start)), ...(color && { "--rec": color }) }}
      aria-label={`Draft ${formatClock(part.start)}–${formatClock(part.end)}`}
      onPointerDown={press("move")}
    >
      <span className="recording-draft-time">{formatClock(part.start)}</span>
      <span className="recording-draft-time">{formatClock(part.end)}</span>
      {movable.start && <span className="grip start" aria-label="Drag to change the start" onPointerDown={press("start")} />}
      {movable.end && <span className="grip end" aria-label="Drag to change the end" onPointerDown={press("end")} />}
    </div>
  );
}
