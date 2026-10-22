import { useId } from "react";
import { useStore } from "../../state/store";
import { RecordingEditor } from "../day/RecordingEditor";
import { Modal } from "../Modal";

/** The recording editor as a sheet: the Week and Month have no timesheet to hold it. */
export function EditorSheet() {
  const editor = useStore((s) => s.editor);
  const id = useId();
  if (!editor) return null;
  return (
    <Modal onClose={() => useStore.getState().closeEditor()} labelledBy={id} className="recording-editor-sheet">
      <RecordingEditor key={editor.mode === "edit" ? editor.id : "new"} id={id} />
    </Modal>
  );
}
