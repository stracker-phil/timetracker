import { type DateJump } from "../../api";
import type { Prefix } from "./parse";
import { useDateJumpsQuery } from "../../queries/calendar";

/** The dates Rust reads in the text (`calendar_jumps`), without a prefix or after `@`; none after another prefix. */
export function useDateJumps(prefix: Prefix, needle: string): DateJump[] {
  const query = prefix === null || prefix === "@" ? needle : null;
  const { data } = useDateJumpsQuery(query);
  // The placeholder would hand back the last dates even while no query asks for them.
  return query === null ? [] : (data ?? []);
}
