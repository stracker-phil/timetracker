import { keepPreviousData, queryOptions, useQuery } from "@tanstack/react-query";
import { api, type Rule, type RuleDay, type RuleInput } from "../api";
import { queryClient } from "./queryClient";

export const RULES_KEY = ["rules"] as const;
export const RULES_TODAY_KEY = ["rulesToday"] as const;
export const RULE_PREVIEW_KEY = ["rulePreview"] as const;

const NO_RULES: Rule[] = [];
const NO_DAYS: RuleDay[] = [];

export const rulesQuery = queryOptions({ queryKey: RULES_KEY, queryFn: api.rulesList, meta: { failure: "Could not load the rules" } });

/** In position order; empty until loaded. */
export const useRules = (): Rule[] => useQuery(rulesQuery).data ?? NO_RULES;

/** The rules as loaded, for code outside components. */
export const currentRules = (): Rule[] => queryClient.getQueryData(RULES_KEY) ?? NO_RULES;

/** Writes the rules as a write left them (or as it will, until it settles). */
export const setRules = (update: (rules: Rule[]) => Rule[]) => queryClient.setQueryData(RULES_KEY, (rules: Rule[] | undefined) => update(rules ?? NO_RULES));

/**
 * Today's matched time and pending suggestions of each active rule; empty when it fails, without a toast. Asked
 * again whenever it shows, since the tracker's new activity arrives without an event.
 */
export const useRulesToday = (): RuleDay[] => useQuery({ queryKey: RULES_TODAY_KEY, queryFn: api.rulesToday, staleTime: 0 }).data ?? NO_DAYS;

/**
 * What `input`'s conditions match over the last `days` days; null asks nothing. Asked again whenever it shows,
 * since the tracker's new activity arrives without an event.
 */
export function useRulePreview(input: RuleInput | null, days: number) {
  return useQuery({
    queryKey: [...RULE_PREVIEW_KEY, input?.conditions ?? null, days],
    queryFn: () => api.rulePreview(input as RuleInput, days),
    enabled: input !== null,
    staleTime: 0,
    placeholderData: keepPreviousData,
  });
}
