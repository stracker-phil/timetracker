import { api, type Recording, type Span } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";
import type { Draft, Editor } from "./editor";

type Create = Slice<unknown>["create"];

interface Deps {
  set: Parameters<Create>[0];
  get: Parameters<Create>[1];
  /** Whether `ticket` is still the latest span ask. */
  isLatest: (ticket: number) => boolean;
  /** The editor's opened draft, which a save updates. */
  opened: { get: () => Draft | null; set: (draft: Draft) => void };
}

/** The times a write changes: the closed spans, and the running timer's open span's start. */
export interface TimesPatch {
  spans?: Span[];
  openStart?: number;
}

export const closedSpans = (recording: Recording): Span[] => recording.spans.flatMap((s) => (s.end === null ? [] : [{ start: s.start, end: s.end }]));

/** The start of the running timer's open span; null when the recording is not running. */
export const openStartOf = (recording: Recording): number | null => recording.spans.find((s) => s.end === null)?.start ?? null;

/**
 * An edited recording's times saving at once: its writes run one after another, a queued one taking in the
 * times asked after it, so the latest ones win.
 */
export function spanWrites({ set, get, isLatest, opened }: Deps) {
  let queue: Promise<void> = Promise.resolve();
  // The writes not done.
  let writes = 0;
  // The times asked per recording that no write has taken yet, with the latest ask's ticket.
  const pending = new Map<string, { patch: TimesPatch; ticket: number }>();
  // Span changes still waiting for Rust's answer, which may then queue a write.
  let asking: Promise<void> = Promise.resolve();

  /** The draft shows the times last saved again. */
  function revert() {
    const draft = get().draft;
    const saved = opened.get();
    if (draft && saved) set({ draft: { ...draft, spans: saved.spans, totalMs: saved.totalMs, openStart: saved.openStart } });
  }

  /** The draft and its opened state take `recording`'s times as saved; the typed fields stay. */
  async function adopt(editor: Editor | null, recording: Recording) {
    const answer = { ...(await api.recordingDraftSpans(closedSpans(recording))), openStart: openStartOf(recording) };
    const draft = get().draft;
    if (get().editor !== editor || !draft) return;
    const saved = opened.get();
    if (saved) opened.set({ ...saved, ...answer });
    set({ editing: recording, draft: { ...draft, ...answer } });
  }

  return {
    revert,
    adopt,

    /** Counts `change` among the span changes `settled` waits for. */
    tracked(change: Promise<void>): Promise<void> {
      asking = Promise.all([asking, change.catch(() => undefined)]).then(() => undefined);
      return change;
    },

    /** The edited recording's id while its times save at once: any existing one, the running timer's too. */
    savingAtOnce(): string | null {
      const { editor, editing } = get();
      return editor?.mode === "edit" && editing ? editing.id : null;
    },

    /**
     * Saves `patch` as the edited recording's times once the writes before it are done (also after the editor
     * closed), with the times asked since; nothing is left to write when an earlier write took them. The draft
     * then shows them unless a later ask came.
     */
    save(id: string, patch: TimesPatch, ticket: number): Promise<void> {
      const editor = get().editor;
      pending.set(id, { patch: { ...pending.get(id)?.patch, ...patch }, ticket });
      writes++;
      const run = queue
        .then(async () => {
          const asked = pending.get(id);
          if (!asked) return;
          pending.delete(id);
          let saved: Recording | null = null;
          try {
            saved = await get().updateRecording(id, asked.patch);
          } catch (e) {
            // A field error: the times have no error line of their own.
            fail(e, "Could not change the times");
          }
          if (!isLatest(asked.ticket)) return;
          if (saved) await adopt(editor, saved);
          else revert();
        })
        .finally(() => writes--);
      queue = run.catch(() => undefined);
      return run;
    },

    /** Settles once the span changes asked so far have their answers and their writes are done. */
    async settled() {
      // A change may ask again while the ones before settle.
      for (let seen: Promise<void> | null = null; seen !== asking; ) {
        seen = asking;
        await seen;
      }
      await queue;
    },

    busy: () => writes > 0,
  };
}
