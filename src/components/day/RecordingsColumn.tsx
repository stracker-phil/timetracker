import type { PointerEvent, Ref } from "react";
import type { RecordingBlock as Block, Span, Suggestion } from "../../api";
import { formatClock } from "../../lib/format";
import type { Draft, Editor } from "../../state/slices/editor";
import { useStore } from "../../state/store";
import { RecordingBlock } from "./RecordingBlock";
import { SuggestionBlock } from "./SuggestionBlock";

interface Props {
  ref?: Ref<HTMLDivElement>;
  recordings: Block[];
  suggestions: Suggestion[];
  y: (ms: number) => number;
  /** The shown day's first and last instant: draft spans are cut to them. */
  bounds: [number, number];
  pxPerMinute: number;
  /** The selection's snapped span, outlined where its recording would go. */
  selection: Span | null;
  /** A press on empty time starts a selection. */
  onPress: (e: PointerEvent<HTMLDivElement>) => void;
}

/** The parts of `spans` inside the shown day, for drawing only (nothing is saved from them). */
function drawnParts(spans: Span[], [first, last]: [number, number]): Span[] {
  return spans.filter((s) => s.end > first && s.start < last).map((s) => ({ start: Math.max(s.start, first), end: Math.min(s.end, last) }));
}

/** The recording or suggestion the open editor edits, whose own blocks give way to its draft spans. */
function editedEntryId(draft: Draft | null, editor: Editor | null): string | null {
  if (!draft || !editor) return null;
  return editor.mode === "edit" ? editor.id : (editor.suggestion?.id ?? null);
}

/** A press on a recording or a suggestion is theirs. */
const onEmptyTime = (target: EventTarget) => target instanceof Element && !target.closest(".recording, [data-suggestion-id], button");

/**
 * The time column: a block per recording span, the suggestions and the open editor's spans; a drag over empty
 * time selects it and opens the editor.
 */
export function RecordingsColumn({ ref, recordings, suggestions, y, bounds, pxPerMinute, selection, onPress }: Props) {
  const focused = useStore((s) => s.focusedRecording);
  const focusedSuggestion = useStore((s) => s.focusedSuggestion);
  const hovered = useStore((s) => s.hoveredRecording);
  const draft = useStore((s) => s.draft);
  const editor = useStore((s) => s.editor);
  const edited = editedEntryId(draft, editor);
  const drafts = drawnParts(draft?.spans ?? [], bounds);
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
          <RecordingBlock key={r.spanId} block={r} y={y} pxPerMinute={pxPerMinute} focused={r.id === focused} hovered={r.id === hovered} look="times" editOnClick />
        ))}
      {drafts.map((s, i) => (
        <div key={i} className="recording-draft" style={{ top: y(s.start), height: Math.max(2, y(s.end) - y(s.start)) }} aria-label={`Draft ${formatClock(s.start)}–${formatClock(s.end)}`}>
          <span className="recording-draft-time">{formatClock(s.start)}</span>
          <span className="recording-draft-time">{formatClock(s.end)}</span>
        </div>
      ))}
    </div>
  );
}
