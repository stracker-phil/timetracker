import type { PointerEvent, Ref } from "react";
import type { RecordingBlock as Block, Span, Suggestion } from "../../api";
import { suggestionParts } from "../../lib/suggestionParts";
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

/** A press on a recording or a suggestion is theirs. */
const onEmptyTime = (target: EventTarget) => target instanceof Element && !target.closest(".recording, [data-suggestion-id], button");

/** The time column: a block per recording span and suggestion, with the open editor's draft spans drawn over the time they replace. */
export function RecordingsColumn({ ref, recordings, suggestions, grid, bounds, selection, onPress }: Props) {
  const { y } = grid;
  const focused = useStore((s) => s.focusedRecording);
  const focusedSuggestion = useStore((s) => s.focusedSuggestion);
  const hovered = useStore((s) => s.hoveredRecording);
  const draft = useStore((s) => s.draft);
  const editor = useStore((s) => s.editor);
  const recording = editor?.mode === "edit" ? editor.id : null;
  const edited = editor?.mode === "create" ? editor.suggestion : undefined;
  const shownSuggestions = draft
    ? suggestionParts(suggestions, draft.spans).map((s) =>
        // The edited running suggestion starts where the draft says.
        s.id === edited?.id && edited.running && draft.openStart !== null ? { ...s, start: draft.openStart } : s,
      )
    : suggestions;
  // The draft's spans stand for the edited recording's closed ones.
  const shownRecordings = draft && recording ? recordings.filter((r) => r.id !== recording || r.running) : recordings;
  const press = (e: PointerEvent<HTMLDivElement>) => {
    if (onEmptyTime(e.target)) onPress(e);
  };
  return (
    <div ref={ref} className="recordings-col" aria-label="Recordings" onPointerDown={press}>
      {selection && <div className="recording-drop" style={{ top: y(selection.start), height: y(selection.end) - y(selection.start) }} aria-hidden="true" />}
      {/* A suggestion the draft splits shows as several parts with its id. */}
      {shownSuggestions.map((s) => (
        <SuggestionBlock key={`${s.id}:${s.start}`} suggestion={s} y={y} focused={s.id === focusedSuggestion} />
      ))}
      {shownRecordings.map((r) => {
        const editing = r.id === recording;
        // The running timer's open span starts where the draft says.
        const block = editing && draft && draft.openStart !== null && !r.clippedStart ? { ...r, start: draft.openStart } : r;
        return <RecordingBlock key={r.spanId} block={block} grid={grid} focused={r.id === focused} hovered={r.id === hovered} editing={editing} />;
      })}
      {draft?.spans.map((s, i) => <DraftBlock key={i} span={s} index={i} grid={grid} bounds={bounds} />)}
    </div>
  );
}
