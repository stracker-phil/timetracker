import { api, toAppError, type Overlap, type Recording, type RecordingInput, type RecordingPatch, type Span, type UndoStatus } from "../../api";
import { formatDuration } from "../../lib/format";
import { fail } from "../requests";
import type { Slice, Store } from "../store";

/** A span picked on the timeline, unsnapped (Rust snaps on save), with the activity blocks it came from. */
export interface Selection {
  start: number;
  end: number;
  blockIds: string[];
}

export type Editor = { mode: "create"; start: number; end: number } | { mode: "edit"; id: string };

export interface RecordingsSlice {
  selection: Selection | null;
  /** The selection as Rust will snap it, while one exists. */
  selectionSnap: Span | null;
  setSelectionSnap: (span: Span | null) => void;
  setSelection: (selection: Selection | null) => void;
  /** Grows the selection to also cover `blockId`'s span. */
  extendSelection: (blockId: string) => void;
  /** The recording keys act on. */
  focusedRecording: string | null;
  focusRecording: (id: string | null) => void;
  editor: Editor | null;
  openEditor: (editor: Editor) => void;
  closeEditor: () => void;
  undoStatus: UndoStatus;
  loadUndoStatus: () => Promise<void>;
  /** Saves; an overlap asks first whether to replace. Rejects with the error when a form field shows it. */
  createRecording: (input: RecordingInput) => Promise<Recording | null>;
  updateRecording: (id: string, patch: RecordingPatch) => Promise<Recording | null>;
  deleteRecording: (id: string) => Promise<void>;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
}

/** A saved recording's length; a running one has none yet. */
const recordedMs = (r: Recording) => (r.end === null ? 0 : r.end - r.start);

const NOTHING_TO_UNDO: UndoStatus = { undoLabel: null, redoLabel: null };

function blockSpan(s: Store, id: string): { start: number; end: number } | null {
  for (const lane of s.dayView?.lanes ?? []) {
    const block = lane.blocks.find((b) => b.id === id);
    if (block) return block;
  }
  return null;
}

export const recordingsSlice: Slice<RecordingsSlice> = {
  create: (set, get) => {
    // One write at a time: a second ⏎ or ⌘Z while one is in flight does nothing.
    let applying = false;

    /** `write` with `overlap: cancel`; on an overlap, asks and runs it again with `replace` (null when declined). */
    async function attempt<T>(write: (overlap: Overlap) => Promise<T>): Promise<T | null> {
      try {
        return await write("cancel");
      } catch (e) {
        const error = toAppError(e);
        if (error.kind !== "overlap") throw e;
        const replace = await confirmReplace((error.details ?? []) as Recording[]);
        return replace ? await write("replace") : null;
      }
    }

    /** One write at a time; null when it did not happen (busy, declined or failed and toasted). */
    function writing<T>(context: string, write: (overlap: Overlap) => Promise<T>): Promise<T | null> {
      return exclusive(context, () => attempt(write));
    }

    async function exclusive<T>(context: string, run: () => Promise<T | null>): Promise<T | null> {
      if (applying) return null;
      applying = true;
      try {
        return await run();
      } catch (e) {
        if (toAppError(e).field) throw e;
        fail(e, context);
        return null;
      } finally {
        applying = false;
        void get().loadUndoStatus();
      }
    }

    function confirmReplace(recordings: Recording[]): Promise<boolean> {
      return new Promise((resolve) => {
        get().openDialog({ type: "overlap", recordings, onReplace: () => resolve(true), onCancel: () => resolve(false) });
      });
    }

    const undoToast = (message: string) => get().toast("success", message, { label: "Undo", run: () => void get().undo() });

    return {
      selection: null,
      selectionSnap: null,
      focusedRecording: null,
      editor: null,
      undoStatus: NOTHING_TO_UNDO,

      setSelection(selection) {
        set(selection ? { selection } : { selection, selectionSnap: null });
      },

      setSelectionSnap(selectionSnap) {
        set({ selectionSnap });
      },

      extendSelection(blockId) {
        const span = blockSpan(get(), blockId);
        if (!span) return;
        const current = get().selection ?? { ...span, blockIds: [] };
        set({
          selection: {
            start: Math.min(current.start, span.start),
            end: Math.max(current.end, span.end),
            blockIds: [...new Set([...current.blockIds, blockId])],
          },
        });
      },

      focusRecording(id) {
        set({ focusedRecording: id });
      },

      openEditor(editor) {
        set({ editor });
      },

      closeEditor() {
        set({ editor: null });
      },

      async loadUndoStatus() {
        try {
          set({ undoStatus: await api.undoStatus() });
        } catch {
          set({ undoStatus: NOTHING_TO_UNDO });
        }
      },

      async createRecording(input) {
        const saved = await writing("Could not save the recording", (overlap) => api.recordingCreate(input, overlap));
        if (saved) {
          const project = get().projects.find((p) => p.project.id === saved.projectId)?.project.name ?? "the project";
          undoToast(`Recorded ${formatDuration(recordedMs(saved))} for ${project}`);
          set({ selection: null, focusedRecording: saved.id });
        }
        return saved;
      },

      async updateRecording(id, patch) {
        const saved = await writing("Could not change the recording", (overlap) => api.recordingUpdate(id, patch, overlap));
        if (saved) undoToast("Recording changed");
        return saved;
      },

      async deleteRecording(id) {
        const deleted = await writing("Could not delete the recording", async () => {
          await api.recordingDelete(id);
          return id;
        });
        if (deleted === null) return;
        undoToast("Recording deleted");
        if (get().focusedRecording === id) set({ focusedRecording: null });
      },

      async undo() {
        // An undo that would overlap a newer recording cannot replace it: the error says so.
        const label = await exclusive("Could not undo", () => api.undo());
        if (label) get().toast("info", `Undid “${label}”`);
      },

      async redo() {
        const label = await exclusive("Could not redo", () => api.redo());
        if (label) get().toast("info", `Redid “${label}”`);
      },
    };
  },
};
