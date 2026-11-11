import type { Span, Suggestion } from "../api";

/** The parts of `span` outside every one of `covers`, in order. */
const overlaps = (a: Span, b: Span) => a.end > b.start && a.start < b.end;

export function uncoveredParts(span: Span, covers: Span[]): Span[] {
  const sorted = covers.filter((c) => overlaps(c, span)).sort((a, b) => a.start - b.start);
  const parts: Span[] = [];
  let from = span.start;
  for (const c of sorted) {
    if (c.start > from) parts.push({ start: from, end: c.start });
    from = Math.max(from, c.end);
  }
  if (from < span.end) parts.push({ start: from, end: span.end });
  return parts;
}

/** `part` of the suggestion; only the part reaching its end keeps a running one running. */
function partOf(s: Suggestion, part: Span): Suggestion {
  if (part.start === s.start && part.end === s.end) return s;
  return { ...s, ...part, durationMs: part.end - part.start, running: s.running && part.end === s.end };
}

/**
 * The suggestions as the open editor's draft `spans` leave them: each part not under a draft span, so time taken
 * into the draft shows as the draft's and time given back shows as the suggestion again. Computed here, not in
 * Rust: the draft lives in the page and changes with every drag (ADR 0044).
 */
export function suggestionParts(suggestions: Suggestion[], spans: Span[]): Suggestion[] {
  return suggestions.flatMap((s) => uncoveredParts(s, spans).map((part) => partOf(s, part)));
}
