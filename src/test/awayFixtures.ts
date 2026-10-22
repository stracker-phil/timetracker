import type { AwayPrompt } from "../api/types";
import { at } from "./dayFixtures";

/** An idle away span from 12:03 to 12:45 of `DAY` (42 min), suggested snapped to 12:05 to 12:45; `patch` overrides fields. */
export function awayPrompt(patch: Partial<AwayPrompt> = {}): AwayPrompt {
  return {
    span: { id: 1, start: at("12:03"), end: at("12:45"), reason: "idle" },
    endedAt: at("12:45"),
    awayMs: 42 * 60_000,
    suggested: { start: at("12:05"), end: at("12:45") },
    ...patch,
  };
}
