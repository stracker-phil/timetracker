import { useEffect, useState } from "react";
import { api, type Recording } from "../../api";
import type { Editor } from "../../state/slices/recordings";
import { fail, useStore } from "../../state/store";

export interface Draft {
  projectId: string | null;
  start: number;
  end: number;
  note: string;
  tagId: string | null;
}

/** The editor's fields: empty when creating, else the recording's with the times of one of its spans. */
export function useRecordingDraft(editor: Editor) {
  const [existing, setExisting] = useState<Recording | null>(null);
  const [spanId, setSpanId] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft | null>(() =>
    editor.mode === "create" ? { projectId: editor.projectId ?? null, start: editor.start, end: editor.end, note: "", tagId: null } : null,
  );

  useEffect(() => {
    if (editor.mode !== "edit") return;
    const shown = useStore.getState().dayView?.recordings.find((r) => r.id === editor.id);
    const block = editor.at ?? shown;
    const wanted = editor.spanId ?? shown?.spanId;
    if (!block) return;
    // The block is cut to the day; the editor edits the whole span.
    api.recordingsList(block.start, block.end).then(
      (found) => {
        const recording = found.find((r) => r.id === editor.id);
        const span = recording?.spans.find((s) => s.id === wanted) ?? recording?.spans[0];
        if (!recording || !span) return;
        setExisting(recording);
        setSpanId(span.id);
        setDraft({ projectId: recording.projectId, start: span.start, end: span.end ?? span.start, note: recording.note, tagId: recording.tagId });
      },
      (e: unknown) => fail(e, "Could not load the recording"),
    );
  }, [editor]);

  const patch = (p: Partial<Draft>) => setDraft((d) => (d ? { ...d, ...p } : d));
  const otherSpans = existing?.spans.filter((s) => s.id !== spanId) ?? [];
  return { existing, spanId, otherSpans, draft, patch };
}
