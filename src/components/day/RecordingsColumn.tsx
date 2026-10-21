import type { Ref } from "react";
import type { RecordingBlock as Block, Span } from "../../api";
import { useStore } from "../../state/store";
import { RecordingBlock } from "./RecordingBlock";

interface Props {
  ref?: Ref<HTMLDivElement>;
  recordings: Block[];
  y: (ms: number) => number;
  pxPerMinute: number;
  /** The selection's snapped span, outlined where its recording would go. */
  selection: Span | null;
}

/** The recordings of the day; releasing a selection drag over it opens the editor (`Timeline` handles the drop). */
export function RecordingsColumn({ ref, recordings, y, pxPerMinute, selection }: Props) {
  const focused = useStore((s) => s.focusedRecording);
  return (
    <div ref={ref} className="recordings-col" aria-label="Recordings">
      {selection && <div className="recording-drop" style={{ top: y(selection.start), height: y(selection.end) - y(selection.start) }} aria-hidden="true" />}
      {recordings.map((r) => (
        <RecordingBlock key={r.id} block={r} y={y} pxPerMinute={pxPerMinute} focused={r.id === focused} />
      ))}
    </div>
  );
}
