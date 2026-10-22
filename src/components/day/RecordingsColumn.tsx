import type { PointerEvent, Ref } from "react";
import type { RecordingBlock as Block, Span, Suggestion } from "../../api";
import type { Draft, Editor } from "../../state/slices/editor";
import { useStore } from "../../state/store";
import { DraftBlock } from "./DraftBlock";
import { RecordingBlock } from "./RecordingBlock";
import { SuggestionBlock } from "./SuggestionBlock";
import type { TickGrid } from "./tickPointer";

interface Props {
  ref?: Ref<HTMLDivElement>;
  recordings: Block[];
  suggestions: Suggestion[];
  grid: TickGrid;
  /** The shown day's first and last instant: draft spans are drawn cut to them. */
  bounds: [number, number];
  /** The selection on the grid, outlined where its recording would go. */
  selection: Span | null;
  /** A press on empty time starts a selection. */
  onPress: (e: PointerEvent<HTMLDivElement>) => void;
}

/** The recording or suggestion the open editor edits, whose own blocks give way to its draft spans. */
function editedEntryId(draft: Draft | null, editor: Editor | null): string | null {
  if (!draft || !editor) return null;
  return editor.mode === "edit" ? editor.id : (editor.suggestion?.id ?? null);
}

/** A press on a recording or a suggestion is theirs. */
const onEmptyTime = (target: EventTarget) => target instanceof Element && !target.closest(".recording, [data-suggestion-id], button");

/**
 * The time column: a block per recording span, the suggestions and the open editor's spans (their edges drag);
 * a drag over empty time selects it and opens the editor.
 */
export function RecordingsColumn({ ref, recordings, suggestions, grid, bounds, selection, onPress }: Props) {
  const { y } = grid;
  const focused = useStore((s) => s.focusedRecording);
  const focusedSuggestion = useStore((s) => s.focusedSuggestion);
  const hovered = useStore((s) => s.hoveredRecording);
  const draft = useStore((s) => s.draft);
  const editor = useStore((s) => s.editor);
  const edited = editedEntryId(draft, editor);
  const press = (e: PointerEvent<HTMLDivElement>) => {
    if (onEmptyTime(e.target)) onPress(e);
  };
  return (
    <div ref={ref} className="recordings-col" aria-label="Recordings" onPointerDown={press}>
      {selection && <div className="recording-drop" style={{ top: y(selection.start), height: y(selection.end) - y(selection.start) }} aria-hidden="true" />}
      {suggestions
        .filter((s) => s.id !== edited)
        .map((s) => (
          <SuggestionBlock key={s.id} suggestion={s} y={y} focused={s.id === focusedSuggestion} />
        ))}
      {recordings
        .filter((r) => r.id !== edited || r.running)
        .map((r) => (
          <RecordingBlock key={r.spanId} block={r} grid={grid} focused={r.id === focused} hovered={r.id === hovered} />
        ))}
      {draft?.spans.map((s, i) => (
        <DraftBlock key={i} span={s} index={i} grid={grid} bounds={bounds} />
      ))}
    </div>
  );
}
