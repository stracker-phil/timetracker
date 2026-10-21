import { api, type DateJump } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { fail } from "../../state/store";
import type { Prefix } from "./parse";

/** The dates Rust reads in the text (`calendar_jumps`), without a prefix or after `@`; none after another prefix. */
export function useDateJumps(prefix: Prefix, needle: string): DateJump[] {
  const query = prefix === null || prefix === "@" ? needle : null;
  const { data } = useApiResult(query === null ? null : `jumps:${query}`, () => api.calendarJumps(query ?? ""), {
    keepData: true,
    onError: (e) => fail(e, "Could not look up the date"),
  });
  // `keepData` would hand back the last dates even while no query asks for them.
  return query === null ? [] : (data ?? []);
}
