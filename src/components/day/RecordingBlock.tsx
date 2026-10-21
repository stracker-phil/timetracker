import { memo, useEffect, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import type { RecordingBlock as Block } from "../../api";
import { formatHm } from "../../lib/format";
import { fail, useStore } from "../../state/store";
import { shadeColor } from "../Accent";
import { pointerDrag } from "./pointerDrag";
import { useSnappedSpan } from "./useSnappedSpan";

type Grip = "move" | "start" | "end";

const MINUTE_MS = 60_000;

/** The span after dragging `grip` by `dy` px, start first. */
function spanAfterDrag(block: Block, grip: Grip, dy: number, pxPerMinute: number): { start: number; end: number } {
  const shift = (dy / pxPerMinute) * MINUTE_MS;
  const start = grip === "end" ? block.start : block.start + shift;
  const end = grip === "start" ? block.end : block.end + shift;
  return { start: Math.min(start, end), end: Math.max(start, end) };
}

interface Props {
  block: Block;
  y: (ms: number) => number;
  pxPerMinute: number;
  focused: boolean;
  /** A moved block released here; returns whether it took the drop (e.g. onto another day). */
  onDrop?: (block: Block, e: PointerEvent) => boolean;
}

/** A recording in the column: click focuses, double-click edits; drag the body to move it, an edge to resize it. */
export const RecordingBlock = memo(function RecordingBlock({ block, y, pxPerMinute, focused, onDrop }: Props) {
  const [drag, setDrag] = useState<{ start: number; end: number } | null>(null);
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

  const press = (grip: Grip) => (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    const { focusRecording, updateRecording } = useStore.getState();
    focusRecording(block.id);
    if (!movable) return;
    pointerDrag(e, {
      move: (dy) => setDrag(spanAfterDrag(block, grip, dy, pxPerMinute)),
      end: (dy, dragged, ev) => {
        setDrag(null);
        if (!dragged) return;
        if (grip === "move" && onDrop?.(block, ev)) return;
        // Other failures were toasted by the store; a field error (a span over 24 h) has no form here.
        void updateRecording(block.id, spanAfterDrag(block, grip, dy, pxPerMinute)).catch((err: unknown) => fail(err, "Could not change the recording"));
      },
    });
  };

  const color = shadeColor(block.color, block.shade);
  const classes = ["recording", focused && "focused", drag && "dragging", block.clippedStart && "clipped-start", block.clippedEnd && "clipped-end", block.running && "running"];
  return (
    <div
      className={classes.filter(Boolean).join(" ")}
      ref={ref}
      data-recording-id={block.id}
      aria-label={`${block.projectName}${block.note ? `: ${block.note}` : ""}`}
      style={{ top, height, "--rec": color } as CSSProperties}
      onPointerDown={press("move")}
      onDoubleClick={() => useStore.getState().openEditor({ mode: "edit", id: block.id, at: { start: block.start, end: block.end } })}
    >
      {movable && <span className="grip start" aria-label="Drag to change the start" onPointerDown={press("start")} />}
      <span className="recording-project">{block.projectName}</span>
      {block.note && <span className="recording-note mono">{block.note}</span>}
      {block.tag && <span className="recording-tag mono">{block.tag}</span>}
      <span className="recording-duration mono">{formatHm(shown.end - shown.start)}</span>
      {movable && <span className="grip end" aria-label="Drag to change the end" onPointerDown={press("end")} />}
    </div>
  );
});
