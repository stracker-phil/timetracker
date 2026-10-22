import { useEffect, useRef, type KeyboardEvent } from "react";
import { Sparkles, Trash2 } from "lucide-react";
import { useStore } from "../../state/store";
import { NoteInput } from "../NoteInput";
import { ProjectPicker } from "../pickers/ProjectPicker";
import { TagPicker } from "../pickers/TagPicker";
import { DurationField, EditorRow, OverlapNotice } from "./EditorParts";
import { EditorSpans } from "./EditorSpans";
import { useEditorSave } from "./useEditorSave";
import { useProposal } from "./useProposal";

const MINUTE_MS = 60_000;

/**
 * The recording editor, inline in the timesheet (a sheet in the Week view, labelled by `id`): note, project, tag,
 * duration and spans; Save (↵ or ⌘↵), Cancel (Escape), Delete (⌘⌫). Rust snaps the
 * times on save. Opened on a suggestion, it is prefilled with it and Save (Approve) approves it as edited.
 */
export function RecordingEditor({ id }: { id?: string }) {
  const editor = useStore((s) => s.editor);
  const draft = useStore((s) => s.draft);
  const editing = useStore((s) => s.editing);
  const overlap = useStore((s) => s.editorOverlap);
  const step = useStore((s) => s.settings?.timeline.snapMinutes ?? 5) * MINUTE_MS;
  const { closeEditor, patchDraft, setDraftTotal } = useStore.getState();
  const { errors, save, remove } = useEditorSave();
  const ref = useRef<HTMLFormElement>(null);
  const proposed = useProposal(draft?.spans ?? null);
  const prefill = editor?.mode === "create" && !editor.projectId;
  const projectId = draft?.projectId ?? null;

  useEffect(() => {
    ref.current?.scrollIntoView?.({ block: "nearest" });
  }, [editor]);

  // A new recording takes the proposed project until one is picked.
  useEffect(() => {
    if (prefill && projectId === null && proposed) useStore.getState().patchDraft({ projectId: proposed });
  }, [prefill, projectId, proposed]);

  // Escape cancels wherever the focus is; an open list or dialog takes it first.
  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape" && !e.defaultPrevented) useStore.getState().closeEditor();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (!editor || !draft) return null;
  const open = editing?.spans.find((s) => s.end === null) ?? null;
  const suggestion = editor.mode === "create" ? editor.suggestion : undefined;

  const onKeyDown = (e: KeyboardEvent) => {
    if (!e.metaKey) return;
    if (e.key === "Enter") {
      e.preventDefault();
      void save();
    } else if (e.key === "Backspace" && editing) {
      e.preventDefault();
      remove();
    }
  };

  return (
    <form
      ref={ref}
      id={id}
      className="recording-editor"
      aria-label={editing ? "Edit recording" : suggestion ? "Edit suggestion" : "New recording"}
      onKeyDown={onKeyDown}
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      {suggestion && (
        <p className="editor-suggested" role="note">
          <Sparkles size="0.923rem" aria-hidden="true" />
          Suggested by {suggestion.ruleName}. Approve records it.
        </p>
      )}
      <div className="editor-note">
        <NoteInput aria-label="Note" placeholder="What did you work on?" autoFocus maxLength={500} value={draft.note} onChange={(note) => patchDraft({ note })} />
        {errors.note && (
          <span className="form-field-error" role="alert">
            {errors.note}
          </span>
        )}
      </div>
      <EditorRow label="Project" error={errors.projectId}>
        <ProjectPicker value={draft.projectId} onChange={(picked) => patchDraft({ projectId: picked })} />
      </EditorRow>
      <EditorRow label="Tag" error={errors.tagId}>
        <TagPicker value={draft.tagId} onChange={(tagId) => patchDraft({ tagId })} />
      </EditorRow>
      <EditorRow label="Duration">
        <DurationField total={draft.totalMs} readOnly={open !== null || draft.spans.length === 0} onChange={setDraftTotal} />
      </EditorRow>
      <EditorSpans spans={draft.spans} step={step} runningSince={open?.start ?? null} error={errors.end ?? errors.start ?? errors.spans} />
      {overlap && <OverlapNotice overlap={overlap} />}
      <div className="editor-actions">
        {editing && (
          <button type="button" className="btn btn-quiet editor-delete" aria-label="Delete" title="Delete (⌘⌫)" onClick={remove}>
            <Trash2 size="1rem" aria-hidden="true" />
          </button>
        )}
        <button type="button" className="btn" title="Cancel (Escape)" onClick={closeEditor}>
          Cancel
        </button>
        <button type="submit" className="btn btn-primary" title={suggestion ? "Approve (↵)" : "Save (↵)"}>
          {suggestion ? "Approve" : "Save"}
        </button>
      </div>
    </form>
  );
}
