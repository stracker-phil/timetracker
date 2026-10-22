import { memo, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import type { RecordingBlock as Block } from "../../api";
import { formatClock } from "../../lib/format";
import { fail, useStore } from "../../state/store";
import { shadeColor } from "../Accent";
import { Card } from "../Card";
import { pointerDrag } from "./pointerDrag";
import { spanAfterDrag, type Grip } from "./spanDrag";
import { useSnappedSpan, type DraggedSpan } from "./useSnappedSpan";

/** Below this height (px) a block shows no times; below `BOTH_TIMES_PX` only its start. */
const TIMES_MIN_PX = 16;
const BOTH_TIMES_PX = 34;

interface Props {
  block: Block;
  y: (ms: number) => number;
  pxPerMinute: number;
  focused: boolean;
  /** Its recording is hovered here or in the timesheet. */
  hovered?: boolean;
}

/**
 * One span of a recording in the Day view's time column, filled in its project's color and showing its start and end:
 * a click focuses and edits it; drag the body to move it, an edge to resize it, both on the snap grid.
 */
export const RecordingBlock = memo(function RecordingBlock({ block, y, pxPerMinute, focused, hovered = false }: Props) {
  const [drag, setDrag] = useState<DraggedSpan | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focused) ref.current?.scrollIntoView?.({ block: "nearest" });
  }, [focused]);
  const ghost = useSnappedSpan(drag);
  const shown = ghost ?? block;
  const top = y(shown.start);
  const height = Math.max(2, y(shown.end) - top);
  // A clipped edge belongs to another day: it does not move here.
  const movable = !block.running && !block.clippedStart && !block.clippedEnd;

  const edit = () => useStore.getState().openEditor({ mode: "edit", id: block.id, at: { start: block.start, end: block.end }, spanId: block.spanId });

  /** Rust snaps and saves the dragged span; other failures were toasted by the store. */
  async function save(dragged: DraggedSpan) {
    try {
      await useStore.getState().updateSpan(block.id, block.spanId, { start: dragged.start, end: dragged.end }, dragged.moving ?? null);
    } catch (err) {
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
    pointerDrag(e, {
      move: (dy) => setDrag(spanAfterDrag(block, grip, dy, pxPerMinute)),
      end: (dy, dragged) => {
        setDrag(null);
        if (!dragged) return edit();
        void save(spanAfterDrag(block, grip, dy, pxPerMinute));
      },
    });
  };

  const classes = ["recording", focused && "focused", hovered && "hovered", drag && "dragging", block.clippedStart && "clipped-start", block.clippedEnd && "clipped-end", block.running && "running"];
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
