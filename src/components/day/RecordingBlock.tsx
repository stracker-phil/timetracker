import { memo, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import type { RecordingBlock as Block, Span } from "../../api";
import { formatClock } from "../../lib/format";
import { fail, useStore } from "../../state/store";
import { shadeColor } from "../Accent";
import { Card } from "../Card";
import { pointerDrag } from "./pointerDrag";
import { sameSpan, spanMoved, spanWithEdge, type Grip } from "./spanDrag";
import { tickPointer, type TickGrid } from "./tickPointer";

/** Below this height (px) a block shows no times; below `BOTH_TIMES_PX` only its start. */
const TIMES_MIN_PX = 16;
const BOTH_TIMES_PX = 34;

interface Props {
  block: Block;
  grid: TickGrid;
  focused: boolean;
  /** Its recording is hovered here or in the timesheet. */
  hovered?: boolean;
}

/** A drag's span; once released, it shows until the block's times differ from those it had before (`from`). */
interface Drag {
  span: Span;
  from?: Span;
}

/**
 * One span of a recording in the Day view's time column, filled in its project's color and showing its start and end:
 * a click focuses and edits it; drag the body to move it, an edge to resize it, both from tick to tick (an edge
 * drag leaves the other edge where it was).
 */
export const RecordingBlock = memo(function RecordingBlock({ block, grid, focused, hovered = false }: Props) {
  const [drag, setDrag] = useState<Drag | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focused) ref.current?.scrollIntoView?.({ block: "nearest" });
  }, [focused]);
  // Rust's new times have arrived: the released drag is done.
  useEffect(() => {
    const now = { start: block.start, end: block.end };
    setDrag((d) => (d?.from && !sameSpan(d.from, now) ? null : d));
  }, [block.start, block.end]);
  const ghost = drag?.span ?? null;
  const shown = ghost ?? block;
  const top = grid.y(shown.start);
  const height = Math.max(2, grid.y(shown.end) - top);
  // A clipped edge belongs to another day: it does not move here.
  const movable = !block.running && !block.clippedStart && !block.clippedEnd && grid.ticks.length > 1;

  const edit = () => useStore.getState().openEditor({ mode: "edit", id: block.id, at: { start: block.start, end: block.end }, spanId: block.spanId });

  /** Rust saves the dragged span as it is; other failures were toasted by the store. */
  async function save(span: Span, grip: Grip) {
    setDrag({ span, from: { start: block.start, end: block.end } });
    try {
      const saved = await useStore.getState().updateSpan(block.id, block.spanId, span, grip === "move" ? null : grip);
      if (!saved) setDrag(null);
    } catch (err) {
      setDrag(null);
      // A field error (a span over 24 h) has no form here.
      fail(err, "Could not change the recording");
    }
  }

  const press = (grip: Grip) => (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    useStore.getState().focusRecording(block.id);
    if (!movable) {
      edit();
      return;
    }
    const tickAt = tickPointer(grid, e.currentTarget);
    const origin = tickAt(e.clientY).index;
    let last: Span = block;
    pointerDrag(e, {
      move: (_dy, ev) => {
        const at = tickAt(ev.clientY).index;
        last = grip === "move" ? spanMoved(grid, block, at - origin) : spanWithEdge(grid.ticks, block, grip, at);
        setDrag({ span: last });
      },
      end: (_dy, dragged) => {
        if (!dragged) {
          setDrag(null);
          return edit();
        }
        if (sameSpan(last, block)) return setDrag(null);
        void save(last, grip);
      },
    });
  };

  const classes = ["recording", focused && "focused", hovered && "hovered", ghost && "dragging", block.clippedStart && "clipped-start", block.clippedEnd && "clipped-end", block.running && "running"];
  const label = `${block.projectName}${block.note ? `: ${block.note}` : ""}`;
  return (
    <Card
      className={classes.filter(Boolean).join(" ")}
      ref={ref}
      data-recording-id={block.id}
      aria-label={label}
      style={{ top, height }}
      color={shadeColor(block.color, block.shade)}
      title={height >= TIMES_MIN_PX ? formatClock(shown.start) : undefined}
      trailing={height >= BOTH_TIMES_PX ? formatClock(shown.end) : undefined}
      onPointerDown={press("move")}
      onPointerEnter={() => useStore.getState().hoverRecording(block.id)}
      onPointerLeave={() => useStore.getState().hoverRecording(null)}
    >
      {movable && <span className="grip start" aria-label="Drag to change the start" onPointerDown={press("start")} />}
      {movable && <span className="grip end" aria-label="Drag to change the end" onPointerDown={press("end")} />}
    </Card>
  );
});
