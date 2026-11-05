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

export const closedSpans = (recording: Recording): Span[] => recording.spans.flatMap((s) => (s.end === null ? [] : [{ start: s.start, end: s.end }]));

/** The running timer's spans are Rust's to end; its editor takes no spans from the timeline. */
export const runningEdited = (editing: Recording | null) => !!editing?.spans.some((s) => s.end === null);

/** An edited recording's times saving at once: its span writes run one after another, the latest one winning. */
export function spanWrites({ set, get, isLatest, opened }: Deps) {
  let queue: Promise<void> = Promise.resolve();
  // `writes` counts the writes not done, `latest` numbers them.
  let writes = 0;
  let latest = 0;
  // Span changes still waiting for Rust's answer, which may then queue a write.
  let asking: Promise<void> = Promise.resolve();

  /** The draft shows the spans last saved again. */
  function revert() {
    const draft = get().draft;
    const saved = opened.get();
    if (draft && saved) set({ draft: { ...draft, spans: saved.spans, totalMs: saved.totalMs } });
  }

  /** The draft and its opened state take `recording`'s spans as saved; the typed fields stay. */
  async function adopt(editor: Editor | null, recording: Recording) {
    const answer = await api.recordingDraftSpans(closedSpans(recording));
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

    /** The edited recording's id while its times save at once: an existing one whose spans are not Rust's to end. */
    savingAtOnce(): string | null {
      const { editor, editing } = get();
      return editor?.mode === "edit" && editing && !runningEdited(editing) ? editing.id : null;
    },

    /**
     * Saves `spans` as the edited recording's once the writes before it are done (also after the editor closed);
     * skipped when a later write holds newer spans. The draft then shows them unless a later ask came.
     */
    save(id: string, spans: Span[], ticket: number): Promise<void> {
      const editor = get().editor;
      const mine = ++latest;
      writes++;
      const run = queue
        .then(async () => {
          if (mine !== latest) return;
          let saved: Recording | null = null;
          try {
            saved = await get().updateRecording(id, { spans });
          } catch (e) {
            // A field error (a span over 24 h): the times have no error line of their own.
            fail(e, "Could not change the times");
          }
          if (!isLatest(ticket)) return;
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
