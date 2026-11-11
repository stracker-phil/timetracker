import type { CSSProperties } from "react";
import type { Suggestion } from "../../api";
import { formatClock, formatDuration } from "../../lib/format";
import { NO_PROJECT } from "../../state/slices/suggestions";
import { useStore } from "../../state/store";
import { shadeColor } from "../Accent";

/** `09:00–10:30`, or `since 09:00` while it runs. */
export const spanLabel = (s: Suggestion) => (s.running ? `since ${formatClock(s.start)}` : `${formatClock(s.start)}–${formatClock(s.end)}`);

interface Props {
  suggestion: Suggestion;
  y: (ms: number) => number;
  focused: boolean;
}

/** A suggestion (or the part of it the open draft leaves) in the time column, hatched: not a recording until approved.
 *  A click opens it in the editor, or adds its time to the open one. A suggested timer's piece that reaches now
 *  carries the pulsing dot. */
export function SuggestionBlock({ suggestion: s, y, focused }: Props) {
  const top = y(s.start);
  const height = Math.max(2, y(s.end) - top);
  const span = spanLabel(s);
  const project = s.projectName ?? NO_PROJECT;
  return (
    <div
      role="button"
      className={["suggestion", focused && "focused", s.running && "running"].filter(Boolean).join(" ")}
      data-suggestion-id={s.id}
      aria-label={`Suggested: ${project} ${span}, by ${s.by}`}
      title={[`Suggested by ${s.by}: ${project}`, span, s.note].filter(Boolean).join("\n")}
      style={{ top, height, "--rec": shadeColor(s.color, s.shade) } as CSSProperties}
      // A press here is the suggestion's; it must not start a selection drag on the column.
      onPointerDown={(e) => e.stopPropagation()}
      onClick={() => useStore.getState().clickSuggestion(s)}
    >
      <span className="suggestion-project">{project}</span>
      <span className="suggestion-by">{s.by}</span>
      <span className="recording-duration">{formatDuration(s.durationMs)}</span>
    </div>
  );
}
