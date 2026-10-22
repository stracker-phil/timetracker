import { useState } from "react";
import { toAppError } from "../../api";
import { useStore } from "../../state/store";
import { typedTimesSettled } from "./TimeField";

/** The editor's Save and Delete, with the field errors a save brought back. */
export function useEditorSave() {
  const [errors, setErrors] = useState<Record<string, string>>({});

  async function save() {
    await typedTimesSettled();
    const { editor, draft, editing, createRecording, updateRecording, closeEditor } = useStore.getState();
    if (!draft) return;
    if (!draft.projectId) {
      setErrors({ projectId: "Pick a project" });
      return;
    }
    setErrors({});
    const fields = { projectId: draft.projectId, note: draft.note, tagId: draft.tagId };
    // The running timer's spans are Rust's to end.
    const running = !!editing?.spans.some((s) => s.end === null);
    try {
      const suggestionId = editor?.mode === "create" ? editor.suggestion?.id : undefined;
      const saved = editing ? await updateRecording(editing.id, running ? fields : { ...fields, spans: draft.spans }) : await createRecording({ ...fields, spans: draft.spans }, suggestionId);
      if (saved) closeEditor();
    } catch (e) {
      const field = toAppError(e).field;
      if (field) setErrors({ [field.path]: field.message });
    }
  }

  function remove() {
    const { editing, closeEditor, deleteRecording } = useStore.getState();
    if (!editing) return;
    closeEditor();
    void deleteRecording(editing.id);
  }

  return { errors, save, remove };
}
