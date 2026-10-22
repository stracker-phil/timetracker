import type { DayView, Suggestion, TimesheetCard } from "../api";

export type SheetEntry = { kind: "card"; id: string; start: number; card: TimesheetCard } | { kind: "suggestion"; id: string; start: number; suggestion: Suggestion };

/** The timesheet in order: the recordings' cards with each suggestion before the card Rust placed it at (`nextCard`). */
export function sheetEntries(view: DayView): SheetEntry[] {
  const before = (cardId: string | null): SheetEntry[] =>
    view.suggestions.filter((s) => s.nextCard === cardId).map((suggestion) => ({ kind: "suggestion", id: suggestion.id, start: suggestion.start, suggestion }));
  return [...view.timesheet.flatMap((card): SheetEntry[] => [...before(card.id), { kind: "card", id: card.id, start: card.start, card }]), ...before(null)];
}
