import { api, type DayView, type DraftSpans, type Recording, type SnapEdge, type Span } from "../../api";
import { findBlock } from "../../lib/blockFocus";
import { fail } from "../requests";
import type { Slice, View } from "../store";

/** The suggestion a new recording approves once saved. */
export interface EditedSuggestion {
  id: string;
  by: string;
}

/**
 * `projectId`, `note`, `tagId`: prefilled for a new recording; `suggestion`: the suggestion its Save approves.
 * `at`: a span of the recording, to find it by; the shown day's block when absent.
 * `spanId`: the span the editor scrolls to; the first one when absent.
 */
export type Editor =
  | { mode: "create"; start: number; end: number; projectId?: string; note?: string; tagId?: string | null; suggestion?: EditedSuggestion }
  | { mode: "edit"; id: string; at?: { start: number; end: number }; spanId?: string };

export interface Draft {
  projectId: string | null;
  /** As Rust returned them (ends past midnight rolled); the running timer's open span is not among them. */
  spans: Span[];
  /** The spans' sum, from Rust. */
  totalMs: number;
  note: string;
  tagId: string | null;
}

/** Recordings a save from the editor would overlap, asked inside the editor. */
export interface EditorOverlap {
  recordings: Recording[];
  resolve: (replace: boolean) => void;
}

export interface EditorSlice {
  editor: Editor | null;
  /** The view the editor was opened in; the Week and Month views show only their own editor as a sheet. */
  editorView: View | null;
  /** The open editor's fields; null until Rust has answered for its spans (and an edited recording has loaded). */
  draft: Draft | null;
  /** The recording the editor edits, once loaded. */
  editing: Recording | null;
  editorOverlap: EditorOverlap | null;
  /** The activity blocks clicked into the open editor's spans, marked as selected on the timeline. */
  draftBlockIds: string[];
  /** A span of the draft while a drag of one of its edges holds it, on ticks: drawn and shown in its fields instead. */
  draftDrag: { index: number; span: Span } | null;
  setDraftDrag: (drag: { index: number; span: Span } | null) => void;
  openEditor: (editor: Editor) => void;
  /** A new recording on Rust's default span: the half hour up to now, on another `day` at the same clock times. */
  openNewRecording: (day?: string) => Promise<void>;
  /** Closes the editor and drops its draft and the selection. */
  closeEditor: () => void;
  patchDraft: (patch: Partial<Pick<Draft, "projectId" | "note" | "tagId">>) => void;
  /**
   * Runs `proceed` once the open editor is out of the way: at once without one, after closing an unchanged one,
   * and for a changed one only after "Discard changes?" is answered with Discard (Keep editing stays).
   */
  leaveEditor: (proceed: () => void) => void;
  /** New times for one span; shown at once, then as Rust rolls them. Resolves once Rust has answered for the new spans. */
  setDraftSpan: (index: number, span: Span) => Promise<void>;
  removeDraftSpan: (index: number) => void;
  /**
   * The span at `index` as a drag of its `moving` edge left it on the timeline (on ticks; the other edge as it
   * was): Rust merges it with the spans it then touches or overlaps. Ends the drag once Rust has answered.
   */
  resizeDraftSpan: (index: number, span: Span, moving: SnapEdge) => Promise<void>;
  /** A span after the last (Add span), placed by Rust. */
  addDraftSpan: () => void;
  /** Moves the last span's end so the spans add up to `totalMs` (Rust's call). */
  setDraftTotal: (totalMs: number) => void;
  /**
   * The selection, snapped by Rust (a drag's is on the grid already): a new editor opens on it; an open editor
   * takes it as another span, merged with the spans it touches or overlaps.
   */
  recordSelection: () => Promise<void>;
  /**
   * A click on an activity block: without an editor, a new one opens on the block's span; with one, the
   * block's span joins its spans, or leaves them when they already cover it (Rust decides and snaps). Removing
   * the last span closes the editor without saving.
   */
  toggleBlock: (blockId: string) => Promise<void>;
  answerOverlap: (replace: boolean) => void;
}

/** The running timer's spans are Rust's to end; its editor takes no spans from the timeline. */
const runningEdited = (editing: Recording | null) => !!editing?.spans.some((s) => s.end === null);

/** The spans of the blocks `ids` that the day shows. */
function spansOfBlocks(view: DayView, ids: string[]): Span[] {
  return ids.flatMap((id) => {
    const block = findBlock(view, id)?.block;
    return block ? [{ start: block.start, end: block.end }] : [];
  });
}

const sameSpans = (a: Span[], b: Span[]) => a.length === b.length && a.every((s, i) => s.start === b[i].start && s.end === b[i].end);

/** Whether a save of `a` would write what `b` holds. */
const sameFields = (a: Draft, b: Draft) => a.projectId === b.projectId && a.note === b.note && a.tagId === b.tagId && sameSpans(a.spans, b.spans);

const closedSpans = (recording: Recording): Span[] => recording.spans.flatMap((s) => (s.end === null ? [] : [{ start: s.start, end: s.end }]));

export const editorSlice: Slice<EditorSlice> = {
  create: (set, get) => {
    // Answers about the spans arrive in any order; only the latest ask counts.
    let asked = 0;
    // The draft as the editor opened it.
    let openedDraft: Draft | null = null;
    const changed = () => {
      const draft = get().draft;
      return draft !== null && openedDraft !== null && !sameFields(draft, openedDraft);
    };

    /** Shows `spans` at once (when given), then Rust's answer for them unless a later ask came first. */
    async function respan(spans: Span[] | null, ask: () => Promise<DraftSpans>) {
      const ticket = ++asked;
      const draft = get().draft;
      if (draft && spans) set({ draft: { ...draft, spans } });
      try {
        const answer = await ask();
        const current = get().draft;
        if (current && ticket === asked) set({ draft: { ...current, ...answer } });
      } catch (e) {
        fail(e, "Could not change the times");
      }
    }

    /** The draft once Rust has answered for its spans, if the editor is still `editor`. */
    async function fill(editor: Editor, fields: Pick<Draft, "projectId" | "note" | "tagId">, spans: Span[]) {
      const ticket = ++asked;
      const answer = await api.recordingDraftSpans(spans);
      if (get().editor !== editor || ticket !== asked) return;
      const draft = { ...fields, ...answer };
      openedDraft = draft;
      set({ draft });
    }

    async function load(editor: Extract<Editor, { mode: "edit" }>) {
      const shown = get().dayView?.recordings.find((r) => r.id === editor.id);
      const block = editor.at ?? shown;
      if (!block) return;
      // The block is cut to the day; the editor edits the whole recording.
      const recording = (await api.recordingsList(block.start, block.end)).find((r) => r.id === editor.id);
      if (!recording || get().editor !== editor) return;
      set({ editing: recording });
      await fill(editor, { projectId: recording.projectId, note: recording.note, tagId: recording.tagId }, closedSpans(recording));
    }

    const spansNow = () => get().draft?.spans ?? [];

    return {
      editor: null,
      editorView: null,
      draft: null,
      editing: null,
      editorOverlap: null,
      draftBlockIds: [],
      draftDrag: null,

      openEditor(editor) {
        get().editorOverlap?.resolve(false);
        openedDraft = null;
        set({ editor, editorView: get().view, draft: null, editing: null, editorOverlap: null, draftBlockIds: [], draftDrag: null });
        const opened =
          editor.mode === "create"
            ? fill(editor, { projectId: editor.projectId ?? null, note: editor.note ?? "", tagId: editor.tagId ?? null }, [{ start: editor.start, end: editor.end }])
            : load(editor);
        opened.catch((e: unknown) => fail(e, "Could not open the recording"));
      },

      async openNewRecording(day) {
        try {
          const { start, end } = await api.recordingDefaultSpan(day);
          get().openEditor({ mode: "create", start, end });
        } catch (e) {
          fail(e, "Could not open a new recording");
        }
      },

      closeEditor() {
        get().editorOverlap?.resolve(false);
        openedDraft = null;
        set({ editor: null, editorView: null, draft: null, editing: null, editorOverlap: null, draftBlockIds: [], draftDrag: null, selection: null, selectionSnap: null });
      },

      leaveEditor(proceed) {
        if (!get().editor) return proceed();
        if (!changed()) {
          get().closeEditor();
          return proceed();
        }
        get().openDialog({
          type: "confirm",
          title: "Discard changes?",
          message: "The recording you are editing has changes that are not saved.",
          confirmLabel: "Discard",
          cancelLabel: "Keep editing",
          onConfirm: () => {
            get().closeEditor();
            proceed();
          },
        });
      },

      patchDraft(patch) {
        const draft = get().draft;
        if (draft) set({ draft: { ...draft, ...patch } });
      },

      setDraftSpan(index, span) {
        const spans = spansNow().map((s, i) => (i === index ? span : s));
        return respan(spans, () => api.recordingDraftSpans(spans));
      },

      removeDraftSpan(index) {
        const spans = spansNow().filter((_, i) => i !== index);
        void respan(spans, () => api.recordingDraftSpans(spans));
      },

      setDraftDrag(draftDrag) {
        set({ draftDrag });
      },

      async resizeDraftSpan(index, span, moving) {
        const spans = spansNow();
        await respan(null, () => api.recordingDraftResize(spans, index, span, moving));
        set({ draftDrag: null });
      },

      addDraftSpan() {
        const spans = spansNow();
        void respan(null, () => api.recordingDraftAdd(spans));
      },

      setDraftTotal(totalMs) {
        const spans = spansNow();
        void respan(null, () => api.recordingDraftTotal(spans, totalMs));
      },

      async recordSelection() {
        const selection = get().selection;
        if (!selection) return;
        const span = await get().snapSelection("Could not open the editor");
        if (!span) return;
        const { editor, draft, editing } = get();
        if (editor && draft) {
          if (runningEdited(editing)) return;
          const spans = draft.spans;
          set({ selection: null, selectionSnap: null });
          return respan(null, () => api.recordingDraftInclude(spans, span));
        }
        get().openEditor({ mode: "create", start: span.start, end: span.end });
        set({ draftBlockIds: selection.blockIds });
      },

      async toggleBlock(blockId) {
        const { editor, draft, editing, dayView, draftBlockIds, setSelection } = get();
        const found = dayView && findBlock(dayView, blockId);
        if (!dayView || !found) return;
        const block = { start: found.block.start, end: found.block.end };
        if (!editor) {
          setSelection({ ...block, blockIds: [blockId] });
          return get().recordSelection();
        }
        if (!draft || runningEdited(editing)) return;
        const kept = draftBlockIds.filter((id) => id !== blockId);
        const ticket = ++asked;
        try {
          const { included, ...answer } = await api.recordingDraftToggle(draft.spans, block, spansOfBlocks(dayView, kept));
          const current = get().draft;
          if (!current || ticket !== asked) return;
          if (answer.spans.length === 0) return get().closeEditor();
          // The blocks' spans now show as the draft's; a selection would stand for one of them only.
          set({ draft: { ...current, ...answer }, draftBlockIds: included ? [...kept, blockId] : kept, selection: null, selectionSnap: null });
        } catch (e) {
          fail(e, "Could not change the times");
        }
      },

      answerOverlap(replace) {
        const overlap = get().editorOverlap;
        set({ editorOverlap: null });
        overlap?.resolve(replace);
      },
    };
  },
};
