import { api, toAppError, type Rule, type RuleInput, type Suggestion } from "../../api";
import { formatDuration } from "../../lib/format";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface RulesSlice {
  /** In position order. */
  rules: Rule[];
  loadRules: () => Promise<void>;
  /** `id: null` creates. Rejects on failure, so a form can show the field error; other errors are toasted. */
  saveRule: (id: string | null, input: RuleInput) => Promise<Rule>;
  deleteRule: (id: string) => Promise<void>;
  /** `ids`: every rule, in the new order. */
  reorderRules: (ids: string[]) => Promise<void>;
  setRuleEnabled: (id: string, enabled: boolean) => Promise<void>;
  /** Records the suggestion on the shown day. */
  acceptSuggestion: (suggestion: Suggestion) => Promise<void>;
  /** Records every suggestion of the rule on the shown day. */
  acceptRuleSuggestions: (ruleId: string) => Promise<void>;
}

export const rulesSlice: Slice<RulesSlice> = {
  create: (set, get) => {
    const undoToast = (message: string) => get().toast("success", message, { label: "Undo", run: () => void get().undo() });
    return {
      rules: [],

      async loadRules() {
        try {
          set({ rules: await api.rulesList() });
        } catch (e) {
          fail(e, "Could not load the rules");
        }
      },

      async saveRule(id, input) {
        try {
          const saved = await api.ruleSave(id, input);
          await get().loadRules();
          return saved;
        } catch (e) {
          if (!toAppError(e).field) fail(e, "Could not save the rule");
          throw e;
        }
      },

      async deleteRule(id) {
        try {
          await api.ruleDelete(id);
          set((s) => ({ rules: s.rules.filter((r) => r.id !== id) }));
        } catch (e) {
          fail(e, "Could not delete the rule");
        }
      },

      async reorderRules(ids) {
        const before = get().rules;
        // Shown in the new order right away; restored when the write fails.
        const byId = new Map(before.map((r) => [r.id, r]));
        set({ rules: ids.flatMap((id) => byId.get(id) ?? []) });
        try {
          set({ rules: await api.rulesReorder(ids) });
        } catch (e) {
          set({ rules: before });
          fail(e, "Could not reorder the rules");
        }
      },

      async setRuleEnabled(id, enabled) {
        try {
          const rule = await api.ruleSetEnabled(id, enabled);
          set((s) => ({ rules: s.rules.map((r) => (r.id === id ? rule : r)) }));
        } catch (e) {
          fail(e, "Could not change the rule");
        }
      },

      async acceptSuggestion(suggestion) {
        try {
          const recording = await api.suggestionAccept(get().day, suggestion.id);
          undoToast(`Recorded ${formatDuration((recording.end ?? recording.start) - recording.start)} for ${suggestion.projectName}`);
        } catch (e) {
          fail(e, "Could not accept the suggestion");
        }
      },

      async acceptRuleSuggestions(ruleId) {
        try {
          const recorded = await api.suggestionsAcceptRule(get().day, ruleId);
          undoToast(recorded.length === 1 ? "Recorded 1 suggestion" : `Recorded ${recorded.length} suggestions`);
        } catch (e) {
          fail(e, "Could not accept the suggestions");
        }
      },
    };
  },
};
