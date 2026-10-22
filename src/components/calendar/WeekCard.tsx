import { memo, type CSSProperties, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { Tag } from "lucide-react";
import type { CalendarEntry } from "../../api";
import { formatDuration } from "../../lib/format";
import { useStore } from "../../state/store";
import { shadeColor } from "../Accent";
import { Card } from "../Card";
import { pointerDrag } from "../day/pointerDrag";
import { editEntry, entryName } from "./entry";

/** The day column under the pointer, if any. */
function dayAt(e: PointerEvent): string | null {
  const el = document.elementFromPoint(e.clientX, e.clientY);
  return el?.closest<HTMLElement>("[data-day]")?.dataset.day ?? null;
}

/** A card being dragged from its day, and the day under the pointer. */
export interface CardDrag {
  id: string;
  from: string;
  over: string | null;
}

interface Props {
  entry: CalendarEntry;
  day: string;
  focused: boolean;
  dragging: boolean;
  /** While the card is dragged; null once released. */
  onDrag: (drag: CardDrag | null) => void;
}

/**
 * A recording in a Week column, as tall as Rust's `heightUnits`: a click focuses it, a double-click or ↵ edits it,
 * ⌫ deletes it. Dropped on another column its time on this day moves there, with ⌥ it is copied.
 */
export const WeekCard = memo(function WeekCard({ entry, day, focused, dragging, onDrag }: Props) {
  const press = (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    useStore.getState().focusRecording(entry.id);
    if (entry.running) return;
    pointerDrag(e, {
      move: (_, ev) => onDrag({ id: entry.id, from: day, over: dayAt(ev) }),
      end: (_, dragged, ev) => {
        onDrag(null);
        const target = dragged ? dayAt(ev) : null;
        if (!target || target === day) return;
        const { moveRecording, copyRecording } = useStore.getState();
        void (ev.altKey ? copyRecording(entry.id, day, target) : moveRecording(entry.id, day, target));
      },
    });
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter") editEntry(entry);
    else if (e.key === "Backspace" || e.key === "Delete") void useStore.getState().deleteRecording(entry.id);
    else return;
    e.preventDefault();
    e.stopPropagation();
  };
  const classes = ["week-card", focused && "focused", dragging && "dragging", entry.running && "running"];
  return (
    <Card
      className={classes.filter(Boolean).join(" ")}
      color={shadeColor(entry.color, entry.shade)}
      style={{ "--units": entry.heightUnits } as CSSProperties}
      role="button"
      tabIndex={0}
      aria-label={entryName(entry)}
      aria-pressed={focused}
      title={entry.projectName}
      subtitle={entry.note || undefined}
      meta={
        <>
          {entry.tag && (
            <>
              <Tag size="0.846rem" aria-hidden="true" />
              {entry.tag}
            </>
          )}
          {entry.spanIds.length > 1 && <span>{entry.spanIds.length} times</span>}
        </>
      }
      trailing={formatDuration(entry.durationMs)}
      onPointerDown={press}
      onDoubleClick={() => editEntry(entry)}
      onKeyDown={onKeyDown}
    />
  );
});
