import { useId, useState, type KeyboardEvent } from "react";
import { toAppError } from "../../api";
import { formatDuration } from "../../lib/format";
import { useStore } from "../../state/store";
import type { Editor } from "../../state/slices/recordings";
import { FormField } from "../Field";
import { Modal } from "../Modal";
import { ProjectPicker } from "../pickers/ProjectPicker";
import { TagPicker } from "../pickers/TagPicker";
import { TimeField } from "./TimeField";
import { useRecordingDraft } from "./useRecordingDraft";

const MINUTE_MS = 60_000;
const DAY_MS = 24 * 60 * MINUTE_MS;

/** Without line breaks: a note is one line (ADR 0018); pasted ones turn into spaces. */
const oneLine = (text: string) => text.replace(/[\r\n]+/g, " ");

/** Create a recording for a span, or edit one: project, times, note and tags. Rust snaps the times on save. */
export function RecordingEditor({ editor }: { editor: Editor }) {
  const step = useStore((s) => s.settings?.timeline.snapMinutes ?? 5) * MINUTE_MS;
  const { closeEditor, createRecording, updateRecording, deleteRecording } = useStore.getState();
  const { existing, draft, patch } = useRecordingDraft(editor);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const ids = { start: useId(), end: useId(), note: useId() };

  if (!draft) return null;
  // An end at or before the start is on the next day.
  const end = draft.end > draft.start ? draft.end : draft.end + DAY_MS;

  async function save() {
    if (!draft?.projectId) {
      setErrors({ projectId: "Pick a project" });
      return;
    }
    setErrors({});
    const fields = { projectId: draft.projectId, start: draft.start, end, note: draft.note, tagIds: draft.tagIds };
    try {
      const saved = existing ? await updateRecording(existing.id, fields) : await createRecording(fields);
      if (saved) closeEditor();
    } catch (e) {
      const field = toAppError(e).field;
      if (field) setErrors({ [field.path]: field.message });
    }
  }

  const remove = () => {
    if (!existing) return;
    closeEditor();
    void deleteRecording(existing.id);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (!e.metaKey) return;
    if (e.key === "Enter") {
      e.preventDefault();
      void save();
    } else if (e.key === "Backspace" && existing) {
      e.preventDefault();
      remove();
    }
  };

  return (
    <Modal onClose={closeEditor} labelledBy="recording-editor-title" className="recording-editor">
      <form
        className="form-contents"
        onKeyDown={onKeyDown}
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <h2 id="recording-editor-title">{existing ? "Edit recording" : "New recording"}</h2>
        <FormField label="Project" error={errors.projectId}>
          <ProjectPicker value={draft.projectId} autoFocus={!existing} onChange={(projectId) => patch({ projectId })} />
        </FormField>
        <div className="recording-times">
          <FormField label="From" htmlFor={ids.start}>
            <TimeField id={ids.start} value={draft.start} step={step} onChange={(start) => patch({ start })} />
          </FormField>
          <FormField label="To" htmlFor={ids.end} error={errors.end}>
            <TimeField id={ids.end} value={draft.end} step={step} onChange={(value) => patch({ end: value })} />
          </FormField>
          <span className="recording-length mono o2">{formatDuration(end - draft.start)}</span>
        </div>
        <FormField label="Note" htmlFor={ids.note} error={errors.note}>
          <input
            id={ids.note}
            className="input"
            // Editing starts at the note; the open project list would hide the project.
            autoFocus={!!existing}
            maxLength={500}
            value={draft.note}
            onChange={(e) => patch({ note: oneLine(e.target.value) })}
            onPaste={(e) => {
              // A text field drops pasted line breaks, joining the words; spaces keep them apart.
              e.preventDefault();
              const field = e.currentTarget;
              const pasted = oneLine(e.clipboardData.getData("text"));
              const [from, to] = [field.selectionStart ?? field.value.length, field.selectionEnd ?? field.value.length];
              patch({ note: field.value.slice(0, from) + pasted + field.value.slice(to) });
            }}
          />
        </FormField>
        <FormField label="Tags" error={errors.tagIds}>
          <TagPicker value={draft.tagIds} onChange={(tagIds) => patch({ tagIds })} />
        </FormField>
        <div className="actions">
          {existing && (
            <button type="button" className="btn actions-start" title="Delete (⌘⌫)" onClick={remove}>
              Delete
            </button>
          )}
          <button type="button" className="btn" onClick={closeEditor}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" title="Save (⌘↵)">
            Save
          </button>
        </div>
      </form>
    </Modal>
  );
}
