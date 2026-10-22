import type { Suggestion } from "../api";
import { shownSuggestions } from "../state/slices/suggestions";
import { useStore } from "../state/store";
import type { Command } from "./registry";
import { registerScope } from "./scopes";

/** The focused suggestion while the Day view shows it and nothing else holds the focus. */
export function focusedSuggestion(): Suggestion | null {
  const s = useStore.getState();
  if (s.view !== "day" || s.focusedSuggestion === null || s.selection !== null || s.focusedRecording !== null) return null;
  return shownSuggestions(s).find((x) => x.id === s.focusedSuggestion) ?? null;
}

/**
 * The focused suggestion's keys (↵, E, ⌫). A sibling of the Day view's scope, so they may share keys with its
 * recording commands: only one of the two holds the focus at a time.
 */
export function registerSuggestionScope() {
  registerScope({ id: "suggestion", parent: "dated", label: "Focused suggestion", isActive: () => focusedSuggestion() !== null });
}

function withFocused(act: (suggestion: Suggestion) => void) {
  const suggestion = focusedSuggestion();
  if (suggestion) act(suggestion);
}

const hasSuggestions = () => useStore.getState().view === "day" && shownSuggestions(useStore.getState()).length > 0;
const allIds = () => shownSuggestions(useStore.getState()).map((s) => s.id);
const store = useStore.getState;

export const SUGGESTION_COMMANDS: Command[] = [
  { id: "suggestion.approve", title: "Approve suggestion", scope: "suggestion", defaultBinding: "Enter", menu: true, run: () => withFocused((s) => void store().approveSuggestion(s)) },
  { id: "suggestion.edit", title: "Edit suggestion", scope: "suggestion", defaultBinding: "E", menu: true, run: () => withFocused((s) => store().editSuggestion(s)) },
  { id: "suggestion.dismiss", title: "Dismiss suggestion", scope: "suggestion", defaultBinding: "Backspace", menu: true, run: () => withFocused((s) => void store().dismissSuggestions([s.id])) },
  { id: "suggestion.approveRule", title: "Approve all suggestions of the rule", scope: "suggestion", menu: true, run: () => withFocused((s) => void store().approveRuleSuggestions(s.ruleId)) },
  { id: "suggestion.approveAll", title: "Approve all suggestions", scope: "day", menu: true, when: hasSuggestions, run: () => void store().approveSuggestions(allIds()) },
  { id: "suggestion.dismissAll", title: "Dismiss all suggestions", scope: "day", menu: true, when: hasSuggestions, run: () => void store().dismissSuggestions(allIds()) },
];
