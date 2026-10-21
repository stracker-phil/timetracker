import type { CSSProperties } from "react";
import { Plus } from "lucide-react";
import type { Suggestion } from "../../api";
import { formatClock, formatHm } from "../../lib/format";
import { useStore } from "../../state/store";
import { shadeColor } from "../Accent";

interface Props {
  suggestion: Suggestion;
  y: (ms: number) => number;
}

/** A rule's proposal in the recordings column. */
export function SuggestionBlock({ suggestion: s, y }: Props) {
  const top = y(s.start);
  const height = Math.max(2, y(s.end) - top);
  const span = `${formatClock(s.start)}–${formatClock(s.end)}`;
  const title = [`${s.ruleName} → ${s.projectName}`, span, s.note, "Click to record, ⌥-click to record all of this rule"].filter(Boolean).join("\n");
  return (
    <button
      type="button"
      className="suggestion"
      data-suggestion-id={s.id}
      aria-label={`Record ${s.projectName} ${span}, suggested by ${s.ruleName}`}
      title={title}
      style={{ top, height, "--rec": shadeColor(s.color, s.shade) } as CSSProperties}
      // A press here accepts; it must not start a selection drag on the column.
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => {
        const { acceptSuggestion, acceptRuleSuggestions } = useStore.getState();
        void (e.altKey ? acceptRuleSuggestions(s.ruleId) : acceptSuggestion(s));
      }}
    >
      <Plus size="0.846rem" aria-hidden="true" />
      <span className="suggestion-project">{s.projectName}</span>
      <span className="suggestion-rule">{s.ruleName}</span>
      <span className="recording-duration mono">{formatHm(s.end - s.start)}</span>
    </button>
  );
}
