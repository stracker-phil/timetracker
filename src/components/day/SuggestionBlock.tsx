import type { CSSProperties } from "react";
import type { Suggestion } from "../../api";
import { formatClock, formatDuration } from "../../lib/format";
import { useStore } from "../../state/store";
import { shadeColor } from "../Accent";

interface Props {
  suggestion: Suggestion;
  y: (ms: number) => number;
  focused: boolean;
}

/** A rule's suggestion in the time column, hatched: not a recording until approved. A click opens it in the editor. */
export function SuggestionBlock({ suggestion: s, y, focused }: Props) {
  const top = y(s.start);
  const height = Math.max(2, y(s.end) - top);
  const span = `${formatClock(s.start)}–${formatClock(s.end)}`;
  return (
    <div
      role="button"
      className={focused ? "suggestion focused" : "suggestion"}
      data-suggestion-id={s.id}
      aria-label={`Suggested: ${s.projectName} ${span}, by ${s.ruleName}`}
      title={[`Suggested by ${s.ruleName}: ${s.projectName}`, span, s.note].filter(Boolean).join("\n")}
      style={{ top, height, "--rec": shadeColor(s.color, s.shade) } as CSSProperties}
      // A press here is the suggestion's; it must not start a selection drag on the column.
      onPointerDown={(e) => e.stopPropagation()}
      onClick={() => useStore.getState().editSuggestion(s)}
    >
      <span className="suggestion-project">{s.projectName}</span>
      <span className="suggestion-rule">{s.ruleName}</span>
      <span className="recording-duration">{formatDuration(s.durationMs)}</span>
    </div>
  );
}
