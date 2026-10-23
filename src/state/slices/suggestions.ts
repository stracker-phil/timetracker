import { api, type Suggestion } from "../../api";
import { formatDuration } from "../../lib/format";
import { focusCleared } from "../focus";
import { fail } from "../requests";
import type { Slice, Store } from "../store";

export interface SuggestionsSlice {
  /** The suggestion keys act on (↵ approve, E edit, ⌫ dismiss). */
  focusedSuggestion: string | null;
  /** Focuses a suggestion, taking the focus off recordings, blocks and the selection. */
  focusSuggestion: (id: string | null) => void;
  /** Records the suggestion on the shown day as it is. */
  approveSuggestion: (suggestion: Suggestion) => Promise<void>;
  /** Records every suggestion of the rule on the shown day. */
  approveRuleSuggestions: (ruleId: string) => Promise<void>;
  /** Records the shown day's suggestions `ids` as one undoable change. */
  approveSuggestions: (ids: string[]) => Promise<void>;
  /** Dismisses the shown day's suggestions `ids` as one undoable change. */
  dismissSuggestions: (ids: string[]) => Promise<void>;
  /** Opens the editor prefilled with the suggestion; its Save approves it as edited. */
  editSuggestion: (suggestion: Suggestion) => void;
}

/** The shown day's suggestions, once its view is loaded. */
export const shownSuggestions = (s: Store) => (s.dayView?.day === s.day ? s.dayView.suggestions : []);

export const suggestionsSlice: Slice<SuggestionsSlice> = {
  create: (set, get) => {
    const undoToast = (message: string) => get().toast("success", message, { label: "Undo", run: () => void get().undo() });

    /** After the focused suggestion was settled, the focus moves on to the next one left, if any. */
    function focusNextAfter(ids: string[]) {
      const focused = get().focusedSuggestion;
      if (focused === null || !ids.includes(focused)) return;
      const shown = shownSuggestions(get());
      const at = shown.findIndex((s) => s.id === focused);
      const next = shown.slice(at + 1).find((s) => !ids.includes(s.id)) ?? shown.slice(0, at).find((s) => !ids.includes(s.id));
      set({ focusedSuggestion: next?.id ?? null });
    }

    return {
      focusedSuggestion: null,

      focusSuggestion(id) {
        set(id === null ? { focusedSuggestion: null } : { ...focusCleared("suggestion"), focusedSuggestion: id });
      },

      async approveSuggestion(suggestion) {
        try {
          const { durationMs } = await api.suggestionApprove(get().day, suggestion.id);
          focusNextAfter([suggestion.id]);
          undoToast(`Recorded ${formatDuration(durationMs)} for ${suggestion.projectName}`);
        } catch (e) {
          fail(e, "Could not approve the suggestion");
        }
      },

      async approveRuleSuggestions(ruleId) {
        try {
          const recorded = await api.suggestionApproveRule(get().day, ruleId);
          focusNextAfter(shownSuggestions(get()).filter((s) => s.ruleIds.includes(ruleId)).map((s) => s.id));
          undoToast(recorded.length === 1 ? "Recorded 1 suggestion" : `Recorded ${recorded.length} suggestions`);
        } catch (e) {
          fail(e, "Could not approve the suggestions");
        }
      },

      async approveSuggestions(ids) {
        if (ids.length === 0) return;
        try {
          const recorded = await api.suggestionsApprove(get().day, ids);
          focusNextAfter(ids);
          undoToast(recorded.length === 1 ? "Recorded 1 suggestion" : `Recorded ${recorded.length} suggestions`);
        } catch (e) {
          fail(e, "Could not approve the suggestions");
        }
      },

      async dismissSuggestions(ids) {
        if (ids.length === 0) return;
        try {
          const dismissed = await api.suggestionsDismiss(get().day, ids);
          focusNextAfter(ids);
          undoToast(dismissed === 1 ? "Dismissed 1 suggestion" : `Dismissed ${dismissed} suggestions`);
        } catch (e) {
          fail(e, "Could not dismiss the suggestions");
        }
      },

      editSuggestion(suggestion) {
        const s = get();
        s.focusSuggestion(suggestion.id);
        s.openEditor({
          mode: "create",
          start: suggestion.start,
          end: suggestion.end,
          projectId: suggestion.projectId,
          note: suggestion.note,
          tagId: suggestion.tagId,
          suggestion: { id: suggestion.id, by: suggestion.by },
        });
      },
    };
  },
};
