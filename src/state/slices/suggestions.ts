import { api, toAppError, type RecordingPatch, type Span, type Suggestion } from "../../api";
import { formatDuration } from "../../lib/format";
import { focusCleared } from "../focus";
import { fail } from "../requests";
import type { Slice } from "../store";
import { shownDayView } from "../../queries/day";

export interface SuggestionsSlice {
  /** The suggestion keys act on (↵ approve, E edit, ⌫ dismiss). */
  focusedSuggestion: string | null;
  /** Focuses a suggestion, taking the focus off recordings, blocks and the selection. */
  focusSuggestion: (id: string | null) => void;
  /** Records the suggestion on the shown day as it is; one without a project opens the editor to pick one. */
  approveSuggestion: (suggestion: Suggestion) => Promise<void>;
  /** Records every suggestion of the rule on the shown day. */
  approveRuleSuggestions: (ruleId: string) => Promise<void>;
  /** Records the shown day's suggestions `ids` that have a project as one undoable change; the rest stay. */
  approveSuggestions: (ids: string[]) => Promise<void>;
  /** Dismisses the shown day's suggestions `ids` as one undoable change. */
  dismissSuggestions: (ids: string[]) => Promise<void>;
  /** Opens the editor prefilled with the suggestion; its Save approves it as edited. */
  editSuggestion: (suggestion: Suggestion) => void;
  /**
   * A click on a suggestion: with the editor open, `part` (the time it shows, all of it by default) joins the
   * draft's spans; else the suggestion opens in the editor.
   */
  clickSuggestion: (suggestion: Suggestion, part?: Span) => void;
  /** Saves a stored suggestion, still a suggestion; false when toasted, rejects when a form field shows the error. */
  updateSuggestion: (storedId: string, patch: RecordingPatch) => Promise<boolean>;
}

/** The shown day's suggestions, once its view is loaded. */
export const shownSuggestions = (): Suggestion[] => shownDayView()?.suggestions ?? [];

/** What a suggestion without a project shows in its place. */
export const NO_PROJECT = "No project";

/** A suggestion without a project waits for the owner to pick one before it can be approved (ADR 0030). */
export const isApprovable = (suggestion: Suggestion) => suggestion.projectId !== null;

/** A running suggestion (a suggested timer) is neither approved nor dismissed until it stops (ADR 0046). */
export const isSettleable = (suggestion: Suggestion) => !suggestion.running;

export const suggestionsSlice: Slice<SuggestionsSlice> = {
  create: (set, get) => {
    const undoToast = (message: string) => get().toast("success", message, { label: "Undo", run: () => void get().undo() });

    /** After the focused suggestion was settled, the focus moves on to the next one left, if any. */
    function focusNextAfter(ids: string[]) {
      const focused = get().focusedSuggestion;
      if (focused === null || !ids.includes(focused)) return;
      const shown = shownSuggestions();
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
        if (!isSettleable(suggestion)) return;
        if (!isApprovable(suggestion)) {
          get().editSuggestion(suggestion);
          return;
        }
        try {
          const { durationMs } = await api.suggestionApprove(get().day, suggestion.id);
          focusNextAfter([suggestion.id]);
          // A suggested timer's piece keeps running, as the running recording.
          undoToast(suggestion.running ? `Timer approved for ${suggestion.projectName}` : `Recorded ${formatDuration(durationMs)} for ${suggestion.projectName}`);
        } catch (e) {
          fail(e, "Could not approve the suggestion");
        }
      },

      async approveRuleSuggestions(ruleId) {
        try {
          const recorded = await api.suggestionApproveRule(get().day, ruleId);
          focusNextAfter(shownSuggestions().filter((s) => s.ruleIds.includes(ruleId)).map((s) => s.id));
          undoToast(recorded.length === 1 ? "Recorded 1 suggestion" : `Recorded ${recorded.length} suggestions`);
        } catch (e) {
          fail(e, "Could not approve the suggestions");
        }
      },

      async approveSuggestions(requested) {
        const ids = shownSuggestions()
          .filter((s) => requested.includes(s.id) && isApprovable(s) && isSettleable(s))
          .map((s) => s.id);
        if (ids.length === 0) return;
        try {
          const recorded = await api.suggestionsApprove(get().day, ids);
          focusNextAfter(ids);
          undoToast(recorded.length === 1 ? "Recorded 1 suggestion" : `Recorded ${recorded.length} suggestions`);
        } catch (e) {
          fail(e, "Could not approve the suggestions");
        }
      },

      async dismissSuggestions(requested) {
        const running = new Set(shownSuggestions().filter((s) => !isSettleable(s)).map((s) => s.id));
        const ids = requested.filter((id) => !running.has(id));
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
        // A suggested timer keeps running: Save writes to its stored suggestion instead of approving it.
        const timer = suggestion.running && suggestion.storedId ? { storedId: suggestion.storedId } : undefined;
        s.switchEditor({
          mode: "create",
          start: suggestion.start,
          end: suggestion.end,
          ...(timer && { openStart: suggestion.start }),
          projectId: suggestion.projectId ?? undefined,
          note: suggestion.note,
          tagId: suggestion.tagId,
          suggestion: { id: suggestion.id, by: suggestion.by, ...(timer && { running: timer }) },
        });
        // A changed editor stays open, and the focus with it.
        const open = get().editor;
        if (open?.mode === "create" && open.suggestion?.id === suggestion.id) s.focusSuggestion(suggestion.id);
      },

      async updateSuggestion(storedId, patch) {
        try {
          await api.suggestionUpdate(storedId, patch);
          undoToast("Suggestion changed");
          return true;
        } catch (e) {
          if (toAppError(e).field) throw e;
          fail(e, "Could not change the suggestion");
          return false;
        }
      },

      clickSuggestion(suggestion, part = suggestion) {
        const s = get();
        if (s.editor && s.draft) void s.includeSpan({ start: part.start, end: part.end });
        else s.editSuggestion(suggestion);
      },
    };
  },
};
