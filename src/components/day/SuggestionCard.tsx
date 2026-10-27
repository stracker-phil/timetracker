import { memo, useEffect, useRef, type CSSProperties } from "react";
import { Check, FolderPlus, Sparkles, X } from "lucide-react";
import type { Suggestion } from "../../api";
import { formatClock, formatDuration } from "../../lib/format";
import { NO_PROJECT, isApprovable } from "../../state/slices/suggestions";
import { useStore } from "../../state/store";
import { AccentDot, shadeColor } from "../Accent";
import { Card } from "../Card";
import { TagLabel } from "../TagLabel";

const MINUTE_MS = 60_000;

interface Props {
  suggestion: Suggestion;
  focused: boolean;
}

/** A suggestion in the timesheet as a ghost card: ✓ approves, ✕ dismisses, a click edits (↵, ⌫, E when focused). */
export const SuggestionCard = memo(function SuggestionCard({ suggestion: s, focused }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focused) ref.current?.scrollIntoView?.({ block: "nearest" });
  }, [focused]);
  const projectName = s.projectName ?? NO_PROJECT;
  const project = s.clientName ? `${projectName} | ${s.clientName}` : projectName;
  const title = s.note || projectName;
  const approve = isApprovable(s)
    ? { label: "Approve", icon: <Check size="1rem" aria-hidden="true" /> }
    : { label: "Pick a project", icon: <FolderPlus size="1rem" aria-hidden="true" /> };
  const subtitle = s.note ? project : s.clientName;
  const span = `${formatClock(s.start)}–${formatClock(s.end)}`;
  const store = useStore.getState;
  return (
    <Card
      ref={ref}
      className={focused ? "sheet-card suggestion-card focused" : "sheet-card suggestion-card"}
      color={shadeColor(s.color, s.shade)}
      style={{ "--card-minutes": Math.round(s.durationMs / MINUTE_MS) } as CSSProperties}
      role="group"
      tabIndex={0}
      aria-label={`Suggested: ${title}, ${project}, ${span}, by ${s.by}`}
      data-suggestion-id={s.id}
      title={title}
      subtitle={
        subtitle && (
          <>
            <AccentDot accent={s.color} shade={s.shade} />
            {subtitle}
          </>
        )
      }
      meta={
        <>
          <Sparkles size="0.846rem" aria-hidden="true" />
          <span className="suggestion-card-by">Suggested by {s.by}</span>
          {s.tag && <TagLabel name={s.tag} emoji={s.tagEmoji} />}
        </>
      }
      trailing={formatDuration(s.durationMs)}
      onFocus={(e) => e.target === e.currentTarget && store().focusSuggestion(s.id)}
      onClick={() => store().editSuggestion(s)}
    >
      <span className="suggestion-card-actions">
        <button
          type="button"
          className="suggestion-card-action"
          aria-label={approve.label}
          title={`${approve.label} (↵)`}
          onClick={(e) => {
            e.stopPropagation();
            void store().approveSuggestion(s);
          }}
        >
          {approve.icon}
        </button>
        <button
          type="button"
          className="suggestion-card-action"
          aria-label="Dismiss"
          title="Dismiss (⌫)"
          onClick={(e) => {
            e.stopPropagation();
            void store().dismissSuggestions([s.id]);
          }}
        >
          <X size="1rem" aria-hidden="true" />
        </button>
      </span>
    </Card>
  );
});
