import { memo, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { Tag } from "lucide-react";
import type { RecordingBlock as Block } from "../../api";
import { formatClock, formatHm } from "../../lib/format";
import { fail, useStore } from "../../state/store";
import { shadeColor } from "../Accent";
import { Card } from "../Card";
import { pointerDrag } from "./pointerDrag";
import { useSnappedSpan } from "./useSnappedSpan";

type Grip = "move" | "start" | "end";

const MINUTE_MS = 60_000;
/** Below this height (px) a block shows no times; below `BOTH_TIMES_PX` only its start. */
const TIMES_MIN_PX = 16;
const BOTH_TIMES_PX = 34;

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
  /** Its recording is hovered here or in the timesheet. */
  hovered?: boolean;
  /** `card`: project, note, tag and duration (the Week view); `times`: only its start and end (the Day view's time column). */
  look?: "card" | "times";
  /** A moved block released here; returns whether it took the drop (e.g. onto another day). */
  onDrop?: (block: Block, e: PointerEvent) => boolean;
}

/**
 * One span of a recording, filled in its project's color: click focuses, double-click edits; drag the body to move
 * it, an edge to resize it.
 */
export const RecordingBlock = memo(function RecordingBlock({ block, y, pxPerMinute, focused, hovered = false, look = "card", onDrop }: Props) {
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
    const { focusRecording, updateSpan } = useStore.getState();
    focusRecording(block.id);
    if (!movable) return;
    pointerDrag(e, {
      move: (dy) => setDrag(spanAfterDrag(block, grip, dy, pxPerMinute)),
      end: (dy, dragged, ev) => {
        setDrag(null);
        if (!dragged) return;
        if (grip === "move" && onDrop?.(block, ev)) return;
        // Other failures were toasted by the store; a field error (a span over 24 h) has no form here.
        void updateSpan(block.id, block.spanId, spanAfterDrag(block, grip, dy, pxPerMinute)).catch((err: unknown) => fail(err, "Could not change the recording"));
      },
    });
  };

  const classes = ["recording", `recording--${look}`, focused && "focused", hovered && "hovered", drag && "dragging", block.clippedStart && "clipped-start", block.clippedEnd && "clipped-end", block.running && "running"];
  const label = `${block.projectName}${block.note ? `: ${block.note}` : ""}`;
  const content =
    look === "times"
      ? { title: height >= TIMES_MIN_PX && formatClock(shown.start), trailing: height >= BOTH_TIMES_PX ? formatClock(shown.end) : undefined }
      : {
          title: block.projectName,
          subtitle: block.note || undefined,
          meta: block.tag && (
            <>
              <Tag size="0.846rem" aria-hidden="true" />
              {block.tag}
            </>
          ),
          trailing: formatHm(shown.end - shown.start),
        };
  return (
    <Card
      className={classes.filter(Boolean).join(" ")}
      ref={ref}
      data-recording-id={block.id}
      aria-label={label}
      style={{ top, height }}
      color={shadeColor(block.color, block.shade)}
      {...content}
      onPointerDown={press("move")}
      onPointerEnter={() => useStore.getState().hoverRecording(block.id)}
      onPointerLeave={() => useStore.getState().hoverRecording(null)}
      onDoubleClick={() => useStore.getState().openEditor({ mode: "edit", id: block.id, at: { start: block.start, end: block.end }, spanId: block.spanId })}
    >
      {movable && <span className="grip start" aria-label="Drag to change the start" onPointerDown={press("start")} />}
      {movable && <span className="grip end" aria-label="Drag to change the end" onPointerDown={press("end")} />}
    </Card>
  );
});
