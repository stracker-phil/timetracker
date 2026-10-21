import { useEffect, useState } from "react";
import { api, type Recording } from "../../api";
import type { Editor } from "../../state/slices/recordings";
import { fail, useStore } from "../../state/store";

export interface Draft {
  projectId: string | null;
  start: number;
  end: number;
  note: string;
  tagIds: string[];
}

/** The editor's fields: empty for a new span, or loaded from the whole recording being edited. */
export function useRecordingDraft(editor: Editor) {
  const [existing, setExisting] = useState<Recording | null>(null);
  const [draft, setDraft] = useState<Draft | null>(() =>
    editor.mode === "create" ? { projectId: null, start: editor.start, end: editor.end, note: "", tagIds: [] } : null,
  );

  useEffect(() => {
    if (editor.mode !== "edit") return;
    const block = useStore.getState().dayView?.recordings.find((r) => r.id === editor.id);
    if (!block) return;
    // The block is cut to the day; the editor edits the whole recording.
    api.recordingsList(block.start, block.end).then(
      (found) => {
        const recording = found.find((r) => r.id === editor.id);
        if (!recording) return;
        setExisting(recording);
        setDraft({ projectId: recording.projectId, start: recording.start, end: recording.end ?? recording.start, note: recording.note, tagIds: recording.tagIds });
      },
      (e: unknown) => fail(e, "Could not load the recording"),
    );
  }, [editor]);

  const patch = (p: Partial<Draft>) => setDraft((d) => (d ? { ...d, ...p } : d));
  return { existing, draft, patch };
}
