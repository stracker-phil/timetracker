import { memo, type CSSProperties, type KeyboardEvent } from "react";
import type { TimesheetCard as CardData } from "../../api";
import { formatDuration } from "../../lib/format";
import { useStore } from "../../state/store";
import { AccentDot, shadeColor } from "../Accent";
import { Card } from "../Card";
import { TagLabel } from "../TagLabel";

const MINUTE_MS = 60_000;

interface Props {
  card: CardData;
  focused: boolean;
  hovered: boolean;
}

/**
 * A recording in the timesheet: the note as title (the project without one), project and client with the
 * project's dot, the tag and the day's total. A click or ↵ focuses and edits it; hovering lights up its spans
 * in the time column.
 */
export const TimesheetCard = memo(function TimesheetCard({ card, focused, hovered }: Props) {
  const project = card.clientName ? `${card.projectName} | ${card.clientName}` : card.projectName;
  const title = card.note || card.projectName;
  const subtitle = card.note ? project : card.clientName;
  const edit = () => {
    const s = useStore.getState();
    s.focusRecording(card.id);
    s.openEditor({ mode: "edit", id: card.id, spanId: card.spanIds[0] });
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key !== "Enter") return;
    e.preventDefault();
    e.stopPropagation();
    edit();
  };
  const classes = ["sheet-card", focused && "focused", hovered && "hovered", card.running && "running"];
  return (
    <Card
      className={classes.filter(Boolean).join(" ")}
      color={shadeColor(card.color, card.shade)}
      style={{ "--card-minutes": Math.round(card.durationMs / MINUTE_MS) } as CSSProperties}
      role="button"
      tabIndex={0}
      aria-label={`${title}, ${project}, ${formatDuration(card.durationMs)}`}
      aria-pressed={focused}
      data-sheet-card={card.id}
      title={title}
      subtitle={
        subtitle && (
          <>
            <AccentDot accent={card.color} shade={card.shade} />
            {subtitle}
          </>
        )
      }
      meta={
        <>
          {card.tag && <TagLabel name={card.tag} emoji={card.tagEmoji} />}
          {card.spanIds.length > 1 && <span>{card.spanIds.length} times</span>}
        </>
      }
      trailing={formatDuration(card.durationMs)}
      onClick={edit}
      onKeyDown={onKeyDown}
      onPointerEnter={() => useStore.getState().hoverRecording(card.id)}
      onPointerLeave={() => useStore.getState().hoverRecording(null)}
    />
  );
});
