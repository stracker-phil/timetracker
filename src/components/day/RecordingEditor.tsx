import { useId, useState, type KeyboardEvent } from "react";
import { NoteInput } from "../NoteInput";
import { toAppError, type RecordingSpan, type Span } from "../../api";
import { formatDuration } from "../../lib/format";
import { spansText } from "../../lib/recordings";
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

/** Every span of a recording, the one `spanId` with the `edited` times; null for a running recording,
 *  whose open span has no end to send (Rust refuses to edit it too). */
function withSpanEdited(spans: RecordingSpan[], spanId: string | null, edited: Span): Span[] | null {
  const closed: Span[] = [];
  for (const s of spans) {
    if (s.end === null) return null;
    closed.push(s.id === spanId ? edited : { start: s.start, end: s.end });
  }
  return closed;
}

/** Create a recording for a span, or edit one: project, times, note and tag. Rust snaps the times on save. */
export function RecordingEditor({ editor }: { editor: Editor }) {
  const step = useStore((s) => s.settings?.timeline.snapMinutes ?? 5) * MINUTE_MS;
  const { closeEditor, createRecording, updateRecording, deleteRecording } = useStore.getState();
  const { existing, spanId, otherSpans, draft, patch } = useRecordingDraft(editor);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const ids = { start: useId(), end: useId(), note: useId() };

  if (!draft) return null;
  const projectGiven = editor.mode === "create" && !!editor.projectId;
  // An end at or before the start is on the next day.
  const end = draft.end > draft.start ? draft.end : draft.end + DAY_MS;

  async function save() {
    if (!draft?.projectId) {
      setErrors({ projectId: "Pick a project" });
      return;
    }
    setErrors({});
    const edited = { start: draft.start, end };
    const spans = existing ? withSpanEdited(existing.spans, spanId, edited) : [edited];
    if (!spans) {
      setErrors({ end: "Stop the timer first" });
      return;
    }
    const fields = { projectId: draft.projectId, spans, note: draft.note, tagId: draft.tagId };
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
        {editor.mode === "create" && editor.warning && (
          <p className="editor-warning" role="note">
            {editor.warning}
          </p>
        )}
        <FormField label="Project" error={errors.projectId}>
          <ProjectPicker value={draft.projectId} autoFocus={!existing && !projectGiven} onChange={(projectId) => patch({ projectId })} />
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
        {otherSpans.length > 0 && <p className="recording-other-spans">Also {spansText(otherSpans)}</p>}
        <FormField label="Note" htmlFor={ids.note} error={errors.note}>
          <NoteInput
            id={ids.note}
            // Editing, or a given project, starts at the note; the open project list would hide the project.
            autoFocus={!!existing || projectGiven}
            maxLength={500}
            value={draft.note}
            onChange={(note) => patch({ note })}
          />
        </FormField>
        <FormField label="Tag" error={errors.tagId}>
          <TagPicker value={draft.tagId} onChange={(tagId) => patch({ tagId })} />
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
