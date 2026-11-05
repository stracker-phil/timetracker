import { useState } from "react";
import { toAppError } from "../../api";
import { useStore } from "../../state/store";
import { typedTimesSettled } from "./TimeField";

/** The editor's Save, Delete and Stop Timer, with the field errors a save brought back. */
export function useEditorSave() {
  const [errors, setErrors] = useState<Record<string, string>>({});

  async function save() {
    await typedTimesSettled();
    // Save writes after the edited recording's pending time saves.
    await useStore.getState().spansSettled();
    const { editor, draft, editing, createRecording, updateRecording, closeEditor } = useStore.getState();
    if (!draft) return;
    if (!draft.projectId) {
      setErrors({ projectId: "Pick a project" });
      return;
    }
    setErrors({});
    const fields = { projectId: draft.projectId, note: draft.note, tagId: draft.tagId };
    // An existing recording's times were saved as they changed.
    try {
      const suggestionId = editor?.mode === "create" ? editor.suggestion?.id : undefined;
      const saved = editing ? await updateRecording(editing.id, fields) : await createRecording({ ...fields, spans: draft.spans }, suggestionId);
      if (!saved) return;
      closeEditor();
    } catch (e) {
      const field = toAppError(e).field;
      if (field) setErrors({ [field.path]: field.message });
    }
  }

  /** Stops the running timer once the times asked so far are saved; the editor stays open on the recording. */
  async function stop() {
    await typedTimesSettled();
    await useStore.getState().spansSettled();
    await useStore.getState().stopTimer();
  }

  function remove() {
    const { editing, closeEditor, deleteRecording } = useStore.getState();
    if (!editing) return;
    closeEditor();
    void deleteRecording(editing.id);
  }

  return { errors, save, remove, stop };
}
