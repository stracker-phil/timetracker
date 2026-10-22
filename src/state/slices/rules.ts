import { api, toAppError, type Rule, type RuleDay, type RuleInput } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface RulesSlice {
  /** In position order. */
  rules: Rule[];
  /** Today's matched time and pending suggestions of each active rule. */
  rulesToday: RuleDay[];
  /** The rules, then their day. */
  loadRules: () => Promise<void>;
  /** `id: null` creates. Rejects on failure, so a form can show the field error; other errors are toasted. */
  saveRule: (id: string | null, input: RuleInput) => Promise<Rule>;
  deleteRule: (id: string) => Promise<void>;
  /** `ids`: every rule, in the new order. */
  reorderRules: (ids: string[]) => Promise<void>;
  /** One place up (-1) or down (1); at either end nothing happens. */
  moveRule: (id: string, by: 1 | -1) => Promise<void>;
  setRuleEnabled: (id: string, enabled: boolean) => Promise<void>;
}

export const rulesSlice: Slice<RulesSlice> = {
  create: (set, get) => {
    return {
      rules: [],
      rulesToday: [],

      async loadRules() {
        try {
          set({ rules: await api.rulesList() });
        } catch (e) {
          fail(e, "Could not load the rules");
          return;
        }
        try {
          set({ rulesToday: await api.rulesToday() });
        } catch {
          set({ rulesToday: [] });
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

      async moveRule(id, by) {
        const ids = get().rules.map((r) => r.id);
        const from = ids.indexOf(id);
        const to = from + by;
        if (from === -1 || to < 0 || to >= ids.length) return;
        [ids[from], ids[to]] = [ids[to], ids[from]];
        await get().reorderRules(ids);
      },

      async setRuleEnabled(id, enabled) {
        try {
          const rule = await api.ruleSetEnabled(id, enabled);
          set((s) => ({ rules: s.rules.map((r) => (r.id === id ? rule : r)) }));
        } catch (e) {
          fail(e, "Could not change the rule");
        }
      },
    };
  },
};
