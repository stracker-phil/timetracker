import type { EntryRef } from "../api";
import type { Store } from "./store";

const SUGGESTION: Partial<Store> = { focusedSuggestion: null };
const OTHERS: Partial<Store> = { focusedRecording: null, focusedBlock: null, detailsBlock: null, selection: null, selectionSnap: null };

/**
 * What taking the Day view's key focus clears, so one thing holds it at a time: a suggestion clears the recording,
 * block and selection focus; any of those clears the suggestion's; another day clears all.
 */
export function focusCleared(by: "suggestion" | "other" | "day"): Partial<Store> {
  if (by === "suggestion") return OTHERS;
  return by === "other" ? SUGGESTION : { ...OTHERS, ...SUGGESTION };
}

/** The key focus on a revealed entry, off everything else. */
export function focusRevealed(entry: EntryRef): Partial<Store> {
  const focus = entry.kind === "recording" ? { focusedRecording: entry.id } : { focusedSuggestion: entry.id };
  return { ...focusCleared("day"), ...focus, revealing: null };
}
