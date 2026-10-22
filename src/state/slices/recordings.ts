import { api, toAppError, type Overlap, type Recording, type RecordingInput, type RecordingPatch, type SnapEdge, type Span, type UndoStatus } from "../../api";
import { formatDuration } from "../../lib/format";
import { recordedMs } from "../../lib/recordings";
import { focusCleared } from "../focus";
import { fail } from "../requests";
import type { Slice, Store } from "../store";

/** A span picked on the timeline, unsnapped (Rust snaps it), with the activity blocks it came from. */
export interface Selection {
  start: number;
  end: number;
  blockIds: string[];
  /** The end a drag moves, which snaps in whole steps away from the other. */
  moving?: SnapEdge;
}

export interface RecordingsSlice {
  selection: Selection | null;
  /** The selection as Rust will snap it, while one exists. */
  selectionSnap: Span | null;
  setSelectionSnap: (span: Span | null) => void;
  setSelection: (selection: Selection | null) => void;
  /** Grows the selection to also cover `blockId`'s span. */
  extendSelection: (blockId: string) => void;
  /** Adds `blockId` to the selected blocks, or takes it out; the selection covers the rest. */
  toggleSelectionBlock: (blockId: string) => void;
  /** The recording keys act on. */
  focusedRecording: string | null;
  focusRecording: (id: string | null) => void;
  /** The recording under the pointer, in the timesheet or the time column: all its spans and its card light up. */
  hoveredRecording: string | null;
  hoverRecording: (id: string | null) => void;
  undoStatus: UndoStatus;
  loadUndoStatus: () => Promise<void>;
  /**
   * Saves; an overlap asks first whether to replace. Rejects with the error when a form field shows it. With
   * `suggestionId`, the recording approves that suggestion of the shown day as edited.
   */
  createRecording: (input: RecordingInput, suggestionId?: string) => Promise<Recording | null>;
  updateRecording: (id: string, patch: RecordingPatch) => Promise<Recording | null>;
  /** New times for one span of the recording as a drag left them; Rust snaps them with `moving` as the dragged end. */
  updateSpan: (id: string, spanId: string, span: Span, moving?: SnapEdge | null) => Promise<Recording | null>;
  /** Moves the span's start and end by whole snap steps each. */
  nudgeSpan: (id: string, spanId: string, startSteps: number, endSteps: number) => Promise<Recording | null>;
  deleteRecording: (id: string) => Promise<void>;
  /** The recording's spans on `fromDay`, else (null) all of them, to `day` at the same time of day. */
  moveRecording: (id: string, fromDay: string | null, day: string) => Promise<void>;
  /** A copy of the recording's spans on `fromDay`, else (null) of all of them, on `day` at the same time of day. */
  copyRecording: (id: string, fromDay: string | null, day: string) => Promise<void>;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
}


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

    /** Asked inside the editor while it is open, else in a dialog. */
    function confirmReplace(recordings: Recording[]): Promise<boolean> {
      return new Promise((resolve) => {
        if (get().editor) return set({ editorOverlap: { recordings, resolve } });
        get().openDialog({ type: "overlap", recordings, onReplace: () => resolve(true), onCancel: () => resolve(false) });
      });
    }

    const undoToast = (message: string) => get().toast("success", message, { label: "Undo", run: () => void get().undo() });

    return {
      selection: null,
      selectionSnap: null,
      focusedRecording: null,
      hoveredRecording: null,
      undoStatus: NOTHING_TO_UNDO,

      setSelection(selection) {
        set(selection ? { selection, ...focusCleared("other") } : { selection, selectionSnap: null });
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

      toggleSelectionBlock(blockId) {
        const ids = get().selection?.blockIds ?? [];
        const kept = ids.includes(blockId) ? ids.filter((id) => id !== blockId) : [...ids, blockId];
        const spans = kept.flatMap((id) => blockSpan(get(), id) ?? []);
        if (spans.length === 0) return set({ selection: null, selectionSnap: null });
        const start = Math.min(...spans.map((s) => s.start));
        const end = Math.max(...spans.map((s) => s.end));
        set({ selection: { start, end, blockIds: kept } });
      },

      focusRecording(id) {
        set({ focusedRecording: id, ...(id !== null && focusCleared("other")) });
      },

      hoverRecording(id) {
        if (get().hoveredRecording !== id) set({ hoveredRecording: id });
      },

      async loadUndoStatus() {
        try {
          set({ undoStatus: await api.undoStatus() });
        } catch {
          set({ undoStatus: NOTHING_TO_UNDO });
        }
      },

      async createRecording(input, suggestionId) {
        const day = get().day;
        const saved = await writing("Could not save the recording", (overlap) =>
          suggestionId ? api.suggestionApproveEdited(day, suggestionId, input, overlap) : api.recordingCreate(input, overlap),
        );
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

      async updateSpan(id, spanId, span, moving = null) {
        const saved = await writing("Could not change the recording", (overlap) => api.recordingUpdateSpan(id, spanId, span, moving, overlap));
        if (saved) undoToast("Recording changed");
        return saved;
      },

      async nudgeSpan(id, spanId, startSteps, endSteps) {
        const saved = await writing("Could not change the recording", (overlap) => api.recordingNudge(id, spanId, startSteps, endSteps, overlap));
        if (saved) undoToast("Recording changed");
        return saved;
      },

      async moveRecording(id, fromDay, day) {
        const moved = await writing("Could not move the recording", (overlap) => api.recordingMove(id, fromDay, day, overlap));
        if (moved) undoToast("Recording moved");
      },

      async copyRecording(id, fromDay, day) {
        const copy = await writing("Could not copy the recording", (overlap) => api.recordingCopy(id, fromDay, day, overlap));
        if (copy) undoToast("Recording copied");
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
