import { api, type DayView, type DraftSpans, type Recording, type SnapEdge, type Span } from "../../api";
import { findBlock } from "../../lib/blockFocus";
import { fail } from "../requests";
import type { Slice, View } from "../store";
import { shownDayView } from "../../queries/day";
import { closedSpans, openStartOf, sameFields, sameSpans, sameTimes, type Draft, type Editor, type EditorOverlap, type TimesPatch } from "./editorDraft";

export type { Draft, EditedSuggestion, Editor, EditorOverlap, TimesPatch } from "./editorDraft";

/** Every change in the editor, its times too, stays in the draft until Save (or Approve); Cancel drops it. */
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
  /**
   * Opens `editor` in place of an unchanged one; a changed one stays open with a toast asking to save or cancel it
   * first. The recording or suggestion already open stays as it is. An opened recording is focused.
   */
  switchEditor: (editor: Editor) => void;
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
  /** New times for one span; shown at once, then as Rust rolls them. Resolves once Rust has answered. */
  setDraftSpan: (index: number, span: Span) => Promise<void>;
  /** A new start for the running timer's open span. */
  setOpenStart: (start: number) => void;
  removeDraftSpan: (index: number) => void;
  /**
   * The span at `index` as a drag of its `moving` edge left it on the timeline (on ticks; the other edge as it
   * was; null when moved whole): Rust merges it with the spans it then touches or overlaps. Ends the drag once
   * Rust has answered.
   */
  resizeDraftSpan: (index: number, span: Span, moving: SnapEdge | null) => Promise<void>;
  /** A span after the last (Add span), placed by Rust. */
  addDraftSpan: () => void;
  /** Moves the last span's end so the spans add up to `totalMs` (Rust's call). */
  setDraftTotal: (totalMs: number) => void;
  /**
   * The selection, snapped by Rust (a drag's is on the grid already): a new editor opens on it; an open editor
   * takes it as another span (`includeSpan`).
   */
  recordSelection: () => Promise<void>;
  /** `span` joins the open editor's spans, merged with the spans it touches or overlaps. */
  includeSpan: (span: Span) => Promise<void>;
  /**
   * A click on an activity block: without an editor, a new one opens on the block's span; with one, the
   * block's span joins its spans, or leaves them when they already cover it (Rust decides and snaps). Removing
   * the last span closes a new recording's editor without saving; a recording keeps its last span.
   */
  toggleBlock: (blockId: string) => Promise<void>;
  /** Settles once Rust has answered every span change asked so far (Save waits for them). */
  spansSettled: () => Promise<void>;
  /** The edited recording's (or running suggestion's) times the draft changed; empty when unchanged or for a new recording. */
  changedTimes: () => TimesPatch;
  /** Saves the edited recording's changed times alone (before Stop Timer); false when the write did not happen. */
  saveEditedTimes: () => Promise<boolean>;
  /**
   * Takes the edited recording's times as Rust now holds them (an undo, a nudge, a stop), keeping the typed fields;
   * times changed in the draft stay.
   */
  syncEdited: () => Promise<void>;
  answerOverlap: (replace: boolean) => void;
}

/** The spans of the blocks `ids` that the day shows. */
function spansOfBlocks(view: DayView, ids: string[]): Span[] {
  return ids.flatMap((id) => {
    const block = findBlock(view, id)?.block;
    return block ? [{ start: block.start, end: block.end }] : [];
  });
}

export const editorSlice: Slice<EditorSlice> = {
  create: (set, get) => {
    // Answers about the spans arrive in any order; only the latest ask counts.
    let asked = 0;
    // The draft as the editor opened it (an edited recording's times as last read from Rust).
    let openedDraft: Draft | null = null;
    // Span changes still waiting for Rust's answer.
    const pending = new Set<Promise<void>>();

    function tracked(change: Promise<void>): Promise<void> {
      const done = change.finally(() => pending.delete(done));
      pending.add(done);
      return done;
    }

    const changed = () => {
      const { draft } = get();
      return draft !== null && openedDraft !== null && !sameFields(draft, openedDraft);
    };

    /** Shows `spans` at once (when given), then Rust's answer for them unless a later ask came first. */
    const respan = (spans: Span[] | null, ask: () => Promise<DraftSpans>) => tracked(respanNow(spans, ask));

    async function respanNow(spans: Span[] | null, ask: () => Promise<DraftSpans>) {
      const ticket = ++asked;
      const draft = get().draft;
      if (draft && spans) set({ draft: { ...draft, spans } });
      try {
        const answer = await ask();
        const current = get().draft;
        if (ticket === asked && current) set({ draft: { ...current, ...answer } });
      } catch (e) {
        fail(e, "Could not change the times");
      }
    }

    /** The draft once Rust has answered for its spans, if the editor is still `editor`. */
    async function fill(editor: Editor, fields: Pick<Draft, "projectId" | "note" | "tagId" | "openStart">, spans: Span[]) {
      const ticket = ++asked;
      const answer = await api.recordingDraftSpans(spans);
      if (get().editor !== editor || ticket !== asked) return;
      const draft = { ...fields, ...answer };
      openedDraft = draft;
      set({ draft });
    }

    async function load(editor: Extract<Editor, { mode: "edit" }>) {
      const shown = shownDayView()?.recordings.find((r) => r.id === editor.id);
      const block = editor.at ?? shown;
      if (!block) return;
      // The block is cut to the day; the editor edits the whole recording.
      const recording = (await api.recordingsList(block.start, block.end)).find((r) => r.id === editor.id);
      if (!recording || get().editor !== editor) return;
      set({ editing: recording });
      await fill(editor, { projectId: recording.projectId, note: recording.note, tagId: recording.tagId, openStart: openStartOf(recording) }, closedSpans(recording));
    }

    /** The draft and its opened state take `recording`'s times; the typed fields and changed times stay. */
    async function adopt(editor: Editor, recording: Recording) {
      const answer = { ...(await api.recordingDraftSpans(closedSpans(recording))), openStart: openStartOf(recording) };
      const draft = get().draft;
      if (get().editor !== editor || !draft || !openedDraft) return;
      const timesKept = !sameTimes(draft, openedDraft);
      openedDraft = { ...openedDraft, ...answer };
      set({ editing: recording, draft: timesKept ? draft : { ...draft, ...answer } });
    }

    const spansNow = () => get().draft?.spans ?? [];

    /** A click on an activity block (`toggleBlock`). */
    async function toggle(blockId: string): Promise<void> {
      const { editor, draft, draftBlockIds, setSelection } = get();
      const dayView = shownDayView();
      const found = dayView && findBlock(dayView, blockId);
      if (!dayView || !found) return;
      const block = { start: found.block.start, end: found.block.end };
      if (!editor) {
        setSelection({ ...block, blockIds: [blockId] });
        return get().recordSelection();
      }
      if (!draft) return;
      const kept = draftBlockIds.filter((id) => id !== blockId);
      const ticket = ++asked;
      try {
        const { included, ...answer } = await api.recordingDraftToggle(draft.spans, block, spansOfBlocks(dayView, kept));
        const current = get().draft;
        if (!current || ticket !== asked) return;
        if (answer.spans.length === 0 && current.openStart === null) {
          // A recording keeps a span; a new one without any closes.
          if (editor.mode === "create") get().closeEditor();
          return;
        }
        // The blocks' spans now show as the draft's; a selection would stand for one of them only.
        set({ draft: { ...current, ...answer }, draftBlockIds: included ? [...kept, blockId] : kept, selection: null, selectionSnap: null });
      } catch (e) {
        fail(e, "Could not change the times");
      }
    }

    function changedTimes(): TimesPatch {
      const { editor, draft } = get();
      const saved = editor?.mode === "edit" || (editor?.mode === "create" && editor.suggestion?.running);
      if (!saved || !draft || !openedDraft) return {};
      return {
        ...(!sameSpans(draft.spans, openedDraft.spans) && { spans: draft.spans }),
        ...(draft.openStart !== null && draft.openStart !== openedDraft.openStart && { openStart: draft.openStart }),
      };
    }

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
            ? fill(
                editor,
                { projectId: editor.projectId ?? null, note: editor.note ?? "", tagId: editor.tagId ?? null, openStart: editor.openStart ?? null },
                // A suggested timer has only its open span.
                editor.openStart === undefined ? [{ start: editor.start, end: editor.end }] : [],
              )
            : load(editor);
        opened.catch((e: unknown) => fail(e, "Could not open the recording"));
      },

      switchEditor(editor) {
        const open = get().editor;
        const same =
          editor.mode === "edit"
            ? open?.mode === "edit" && open.id === editor.id
            : !!editor.suggestion && open?.mode === "create" && open.suggestion?.id === editor.suggestion.id;
        if (same) return;
        if (changed()) {
          get().toast("info", "Save or cancel the open recording first");
          return;
        }
        get().closeEditor();
        get().openEditor(editor);
        if (editor.mode === "edit") get().focusRecording(editor.id);
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

      setOpenStart(start) {
        const draft = get().draft;
        if (draft && draft.openStart !== null) set({ draft: { ...draft, openStart: start } });
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
        const { editor, draft } = get();
        if (editor && draft) {
          set({ selection: null, selectionSnap: null });
          return get().includeSpan(span);
        }
        get().openEditor({ mode: "create", start: span.start, end: span.end });
        set({ draftBlockIds: selection.blockIds });
      },

      includeSpan(span) {
        const spans = spansNow();
        return respan(null, () => api.recordingDraftInclude(spans, span));
      },

      toggleBlock: (blockId) => tracked(toggle(blockId)),

      async spansSettled() {
        // A change may ask again while the ones before settle.
        while (pending.size > 0) await Promise.allSettled([...pending]);
      },

      changedTimes,

      async saveEditedTimes() {
        const { editor, editing } = get();
        const patch = changedTimes();
        if (!editor || !editing || Object.keys(patch).length === 0) return true;
        const saved = await get().updateRecording(editing.id, patch);
        if (!saved) return false;
        const draft = get().draft;
        if (openedDraft && draft) openedDraft = { ...openedDraft, spans: draft.spans, totalMs: draft.totalMs, openStart: draft.openStart };
        await adopt(editor, saved);
        return true;
      },

      async syncEdited() {
        const { editor, editing } = get();
        if (editor?.mode !== "edit" || !editing?.spans.length || pending.size > 0) return;
        const start = Math.min(...editing.spans.map((s) => s.start));
        const end = Math.max(...editing.spans.map((s) => s.end ?? Date.now()));
        try {
          const fresh = (await api.recordingsList(start, end)).find((r) => r.id === editing.id);
          if (!fresh || get().editing !== editing || sameSpans(fresh.spans, editing.spans)) return;
          await adopt(editor, fresh);
        } catch (e) {
          fail(e, "Could not refresh the recording");
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
