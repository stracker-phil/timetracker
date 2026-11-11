import type { Recording, Span } from "../../api";

/** The suggestion a new recording approves once saved; a running one (a suggested timer) is saved as a suggestion instead. */
export interface EditedSuggestion {
  id: string;
  by: string;
  /** The stored suggestion a running one saves to. */
  running?: { storedId: string };
}

/**
 * `projectId`, `note`, `tagId`: prefilled for a new recording; `suggestion`: the suggestion its Save approves.
 * `openStart`: a running suggestion's open span starts there, with no closed spans.
 * `at`: a span of the recording, to find it by; the shown day's block when absent.
 * `spanId`: the span the editor scrolls to; the first one when absent.
 */
export type Editor =
  | { mode: "create"; start: number; end: number; openStart?: number; projectId?: string; note?: string; tagId?: string | null; suggestion?: EditedSuggestion }
  | { mode: "edit"; id: string; at?: { start: number; end: number }; spanId?: string };

export interface Draft {
  projectId: string | null;
  /** As Rust returned them (ends past midnight rolled); the running timer's open span is not among them. */
  spans: Span[];
  /** The spans' sum, from Rust. */
  totalMs: number;
  /** The running timer's open span's start (its end is Rust's, at stop); null for any other recording. */
  openStart: number | null;
  note: string;
  tagId: string | null;
}

/** Recordings a save from the editor would overlap, asked inside the editor. */
export interface EditorOverlap {
  recordings: Recording[];
  resolve: (replace: boolean) => void;
}

/** The times a save changes: the closed spans, and the running timer's open span's start. */
export interface TimesPatch {
  spans?: Span[];
  openStart?: number;
}

export const closedSpans = (recording: Recording): Span[] => recording.spans.flatMap((s) => (s.end === null ? [] : [{ start: s.start, end: s.end }]));

/** The start of the running timer's open span; null when the recording is not running. */
export const openStartOf = (recording: Recording): number | null => recording.spans.find((s) => s.end === null)?.start ?? null;

type Times = { start: number; end: number | null };
export const sameSpans = (a: Times[], b: Times[]) => a.length === b.length && a.every((s, i) => s.start === b[i].start && s.end === b[i].end);

export const sameTimes = (a: Draft, b: Draft) => sameSpans(a.spans, b.spans) && a.openStart === b.openStart;

/** Whether a save of `a` would write what `b` holds. */
export const sameFields = (a: Draft, b: Draft) => a.projectId === b.projectId && a.note === b.note && a.tagId === b.tagId && sameTimes(a, b);
