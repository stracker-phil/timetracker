import { useState } from "react";
import { toAppError } from "../../api";
import { useStore } from "../../state/store";
import { typedTimesSettled } from "./TimeField";

/** The editor's Save, Delete and Stop Timer, with the field errors a save brought back. */
export function useEditorSave() {
  const [errors, setErrors] = useState<Record<string, string>>({});
  const showFieldError = (e: unknown) => {
    const field = toAppError(e).field;
    if (field) setErrors({ [field.path]: field.message });
  };

  async function save() {
    await typedTimesSettled();
    await useStore.getState().spansSettled();
    const s = useStore.getState();
    const { editor, draft, editing } = s;
    if (!draft) return;
    if (!draft.projectId) {
      setErrors({ projectId: "Pick a project" });
      return;
    }
    setErrors({});
    const fields = { projectId: draft.projectId, note: draft.note, tagId: draft.tagId };
    try {
      const suggestion = editor?.mode === "create" ? editor.suggestion : undefined;
      let saved;
      if (suggestion?.running) {
        // A running suggestion has no closed spans of its own here; added ones go to Rust, which refuses them.
        const spans = draft.spans.length > 0 ? { spans: draft.spans } : {};
        saved = await s.updateSuggestion(suggestion.running.storedId, { ...fields, ...s.changedTimes(), ...spans });
      } else if (editing) {
        saved = await s.updateRecording(editing.id, { ...fields, ...s.changedTimes() });
      } else {
        saved = await s.createRecording({ ...fields, spans: draft.spans }, suggestion?.id);
      }
      if (saved) s.closeEditor();
    } catch (e) {
      showFieldError(e);
    }
  }

  /** Saves the changed times, then stops the running timer; the editor stays open on the recording. */
  async function stop() {
    await typedTimesSettled();
    await useStore.getState().spansSettled();
    try {
      if (!(await useStore.getState().saveEditedTimes())) return;
    } catch (e) {
      showFieldError(e);
      return;
    }
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
