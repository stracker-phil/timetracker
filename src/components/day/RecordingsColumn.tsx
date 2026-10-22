import type { Ref } from "react";
import type { RecordingBlock as Block, Span, Suggestion } from "../../api";
import { useStore } from "../../state/store";
import { RecordingBlock } from "./RecordingBlock";
import { SuggestionBlock } from "./SuggestionBlock";

interface Props {
  ref?: Ref<HTMLDivElement>;
  recordings: Block[];
  suggestions: Suggestion[];
  y: (ms: number) => number;
  pxPerMinute: number;
  /** The selection's snapped span, outlined where its recording would go. */
  selection: Span | null;
}

/** The time column: a block per recording span and the suggestions; a selection released over it opens the editor. */
export function RecordingsColumn({ ref, recordings, suggestions, y, pxPerMinute, selection }: Props) {
  const focused = useStore((s) => s.focusedRecording);
  const hovered = useStore((s) => s.hoveredRecording);
  return (
    <div ref={ref} className="recordings-col" aria-label="Recordings">
      {selection && <div className="recording-drop" style={{ top: y(selection.start), height: y(selection.end) - y(selection.start) }} aria-hidden="true" />}
      {suggestions.map((s) => (
        <SuggestionBlock key={s.id} suggestion={s} y={y} />
      ))}
      {recordings.map((r) => (
        <RecordingBlock key={r.spanId} block={r} y={y} pxPerMinute={pxPerMinute} focused={r.id === focused} hovered={r.id === hovered} look="times" />
      ))}
    </div>
  );
}
