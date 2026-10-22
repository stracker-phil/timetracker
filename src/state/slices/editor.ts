import { api, type DraftSpans, type Recording, type Span } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

/**
 * `projectId`: preselected for a new recording; `warning`: shown above the fields.
 * `at`: a span of the recording, to find it by; the shown day's block when absent.
 * `spanId`: the span the editor scrolls to; the first one when absent.
 */
export type Editor =
  | { mode: "create"; start: number; end: number; projectId?: string; warning?: string }
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
  /** The open editor's fields; null until Rust has answered for its spans (and an edited recording has loaded). */
  draft: Draft | null;
  /** The recording the editor edits, once loaded. */
  editing: Recording | null;
  editorOverlap: EditorOverlap | null;
  openEditor: (editor: Editor) => void;
  /** Closes the editor and drops its draft and the selection. */
  closeEditor: () => void;
  patchDraft: (patch: Partial<Pick<Draft, "projectId" | "note" | "tagId">>) => void;
  /** New times for one span; shown at once, then as Rust rolls them. */
  setDraftSpan: (index: number, span: Span) => void;
  removeDraftSpan: (index: number) => void;
  /** A span after the last (Add span), placed by Rust. */
  addDraftSpan: () => void;
  /** Moves the last span's end so the spans add up to `totalMs` (Rust's call). */
  setDraftTotal: (totalMs: number) => void;
  /**
   * The selection, snapped by Rust, as the editor's span: a new editor opens on it, an open new one moves
   * to it; `add` adds it as another span of the open editor's recording.
   */
  recordSelection: (add?: boolean) => Promise<void>;
  answerOverlap: (replace: boolean) => void;
}

const closedSpans = (recording: Recording): Span[] => recording.spans.flatMap((s) => (s.end === null ? [] : [{ start: s.start, end: s.end }]));

export const editorSlice: Slice<EditorSlice> = {
  create: (set, get) => {
    // Answers about the spans arrive in any order; only the latest ask counts.
    let asked = 0;

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
      if (get().editor === editor && ticket === asked) set({ draft: { ...fields, ...answer } });
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
      draft: null,
      editing: null,
      editorOverlap: null,

      openEditor(editor) {
        get().editorOverlap?.resolve(false);
        set({ editor, draft: null, editing: null, editorOverlap: null });
        const opened =
          editor.mode === "create" ? fill(editor, { projectId: editor.projectId ?? null, note: "", tagId: null }, [{ start: editor.start, end: editor.end }]) : load(editor);
        opened.catch((e: unknown) => fail(e, "Could not open the recording"));
      },

      closeEditor() {
        get().editorOverlap?.resolve(false);
        set({ editor: null, draft: null, editing: null, editorOverlap: null, selection: null, selectionSnap: null });
      },

      patchDraft(patch) {
        const draft = get().draft;
        if (draft) set({ draft: { ...draft, ...patch } });
      },

      setDraftSpan(index, span) {
        const spans = spansNow().map((s, i) => (i === index ? span : s));
        void respan(spans, () => api.recordingDraftSpans(spans));
      },

      removeDraftSpan(index) {
        const spans = spansNow().filter((_, i) => i !== index);
        void respan(spans, () => api.recordingDraftSpans(spans));
      },

      addDraftSpan() {
        const spans = spansNow();
        void respan(null, () => api.recordingDraftAdd(spans));
      },

      setDraftTotal(totalMs) {
        const spans = spansNow();
        void respan(null, () => api.recordingDraftTotal(spans, totalMs));
      },

      async recordSelection(add = false) {
        const selection = get().selection;
        if (!selection) return;
        let span: Span;
        try {
          span = await api.recordingSnap(selection.start, selection.end, selection.moving ?? null);
        } catch (e) {
          fail(e, "Could not open the editor");
          return;
        }
        const { editor, draft } = get();
        if (add && editor && draft) {
          const spans = [...draft.spans, span];
          return respan(spans, () => api.recordingDraftSpans(spans));
        }
        if (editor?.mode === "create" && draft) {
          set({ editor: { ...editor, start: span.start, end: span.end } });
          return respan([span], () => api.recordingDraftSpans([span]));
        }
        get().openEditor({ mode: "create", start: span.start, end: span.end });
      },

      answerOverlap(replace) {
        const overlap = get().editorOverlap;
        set({ editorOverlap: null });
        overlap?.resolve(replace);
      },
    };
  },
};
