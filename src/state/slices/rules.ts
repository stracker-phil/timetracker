import { api, toAppError, type Rule, type RuleInput } from "../../api";
import { refreshed } from "../../queries/queryClient";
import { currentRules, RULES_KEY, RULES_TODAY_KEY, setRules } from "../../queries/rules";
import { fail } from "../requests";
import type { Slice } from "../store";

/** The writes of the rules `useRules` reads (`src/queries/rules.ts`). */
export interface RulesSlice {
  /** `id: null` creates; settles once the rules have refetched. Rejects on failure, so a form can show the field error; other errors are toasted. */
  saveRule: (id: string | null, input: RuleInput) => Promise<Rule>;
  deleteRule: (id: string) => Promise<void>;
  /** `ids`: every rule, in the new order. */
  reorderRules: (ids: string[]) => Promise<void>;
  /** One place up (-1) or down (1); at either end nothing happens. */
  moveRule: (id: string, by: 1 | -1) => Promise<void>;
  setRuleEnabled: (id: string, enabled: boolean) => Promise<void>;
}

export const rulesSlice: Slice<RulesSlice> = {
  create: (_set, get) => ({
    async saveRule(id, input) {
      try {
        const saved = await api.ruleSave(id, input);
        await refreshed(RULES_KEY, RULES_TODAY_KEY);
        return saved;
      } catch (e) {
        if (!toAppError(e).field) fail(e, "Could not save the rule");
        throw e;
      }
    },

    async deleteRule(id) {
      try {
        await api.ruleDelete(id);
        setRules((rules) => rules.filter((r) => r.id !== id));
      } catch (e) {
        fail(e, "Could not delete the rule");
      }
    },

    async reorderRules(ids) {
      const before = currentRules();
      // Shown in the new order right away; restored when the write fails.
      const byId = new Map(before.map((r) => [r.id, r]));
      setRules(() => ids.flatMap((id) => byId.get(id) ?? []));
      try {
        const reordered = await api.rulesReorder(ids);
        setRules(() => reordered);
      } catch (e) {
        setRules(() => before);
        fail(e, "Could not reorder the rules");
      }
    },

    async moveRule(id, by) {
      const ids = currentRules().map((r) => r.id);
      const from = ids.indexOf(id);
      const to = from + by;
      if (from === -1 || to < 0 || to >= ids.length) return;
      [ids[from], ids[to]] = [ids[to], ids[from]];
      await get().reorderRules(ids);
    },

    async setRuleEnabled(id, enabled) {
      try {
        const rule = await api.ruleSetEnabled(id, enabled);
        setRules((rules) => rules.map((r) => (r.id === id ? rule : r)));
      } catch (e) {
        fail(e, "Could not change the rule");
      }
    },
  }),
};
