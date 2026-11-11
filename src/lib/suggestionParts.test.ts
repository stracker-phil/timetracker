import { describe, expect, it } from "vitest";
import { at } from "../test/dayFixtures";
import { suggestion } from "../test/ruleFixtures";
import { suggestionParts, uncoveredParts } from "./suggestionParts";

const span = (from: string, to: string) => ({ start: at(from), end: at(to) });
const MINUTE_MS = 60_000;

describe("uncoveredParts", () => {
  const whole = span("09:00", "10:00");

  it("returns the whole span when nothing covers it", () => {
    expect(uncoveredParts(whole, [])).toEqual([whole]);
  });

  it("ignores covers outside the span, also those that only touch it", () => {
    expect(uncoveredParts(whole, [span("08:00", "09:00"), span("10:00", "11:00")])).toEqual([whole]);
  });

  it("returns nothing when a cover spans it all", () => {
    expect(uncoveredParts(whole, [span("08:30", "10:30")])).toEqual([]);
  });

  it.each([
    ["the start", [span("08:30", "09:20")], [span("09:20", "10:00")]],
    ["the end", [span("09:40", "10:30")], [span("09:00", "09:40")]],
    ["the middle", [span("09:20", "09:40")], [span("09:00", "09:20"), span("09:40", "10:00")]],
  ])("keeps what is left of a span a cover takes %s", (_where, covers, expected) => {
    expect(uncoveredParts(whole, covers)).toEqual(expected);
  });

  it("returns the gaps between several covers in order, whatever order the covers come in", () => {
    const covers = [span("09:40", "09:50"), span("09:10", "09:20")];
    expect(uncoveredParts(whole, covers)).toEqual([span("09:00", "09:10"), span("09:20", "09:40"), span("09:50", "10:00")]);
  });

  it("does not split at covers that overlap each other", () => {
    expect(uncoveredParts(whole, [span("09:10", "09:30"), span("09:20", "09:40")])).toEqual([span("09:00", "09:10"), span("09:40", "10:00")]);
  });
});

describe("suggestionParts", () => {
  const planned = suggestion({ ruleIds: ["rule-1"], ...span("09:00", "10:00") });
  const other = suggestion({ ruleIds: ["rule-2"], ...span("11:00", "11:30") });

  it("returns an uncut suggestion as the same object", () => {
    const parts = suggestionParts([planned, other], [span("13:00", "14:00")]);
    expect(parts).toHaveLength(2);
    expect(parts[0]).toBe(planned);
    expect(parts[1]).toBe(other);
  });

  it("drops a suggestion the spans cover entirely", () => {
    expect(suggestionParts([planned, other], [span("08:00", "10:30")])).toEqual([other]);
  });

  it("cuts a suggestion into the parts outside the spans, with their own times and duration", () => {
    const parts = suggestionParts([planned], [span("09:20", "09:40")]);
    expect(parts).toEqual([
      { ...planned, ...span("09:00", "09:20"), durationMs: 20 * MINUTE_MS },
      { ...planned, ...span("09:40", "10:00"), durationMs: 20 * MINUTE_MS },
    ]);
  });

  it("keeps the other fields of a cut part", () => {
    const [part] = suggestionParts([planned], [span("09:30", "10:00")]);
    expect(part).toMatchObject({ id: planned.id, projectId: planned.projectId, by: planned.by });
  });

  it("marks a part running only when it keeps the suggestion's end", () => {
    const timer = suggestion({ ruleIds: ["rule-1"], running: true, ...span("09:00", "10:00") });
    const parts = suggestionParts([timer], [span("09:20", "09:40")]);
    expect(parts.map((p) => p.running)).toEqual([false, true]);
  });

  it("keeps the order of the suggestions", () => {
    const parts = suggestionParts([planned, other], [span("09:20", "09:40")]);
    expect(parts.map((p) => p.start)).toEqual([at("09:00"), at("09:40"), at("11:00")]);
  });
});
