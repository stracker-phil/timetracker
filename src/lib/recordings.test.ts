import { describe, expect, it } from "vitest";
import { at } from "../test/dayFixtures";
import { recording } from "../test/recordingFixtures";
import { recordedMs, spansText } from "./recordings";

describe("recordedMs", () => {
  it("sums the lengths of all spans", () => {
    const r = recording({
      spans: [
        { id: "a", start: at("14:25"), end: at("15:00") },
        { id: "b", start: at("16:05"), end: at("20:15") },
      ],
    });
    expect(recordedMs(r)).toBe((35 + 250) * 60_000);
  });

  it("counts a single span", () => {
    expect(recordedMs(recording())).toBe(45 * 60_000);
  });

  it("leaves out the open span of a running timer", () => {
    const r = recording({
      spans: [
        { id: "a", start: at("09:00"), end: at("09:30") },
        { id: "b", start: at("10:00"), end: null },
      ],
    });
    expect(recordedMs(r)).toBe(30 * 60_000);
  });
});

describe("spansText", () => {
  it("joins the spans as clock ranges", () => {
    expect(
      spansText([
        { start: at("14:25"), end: at("15:00") },
        { start: at("16:05"), end: at("20:15") },
      ]),
    ).toBe("14:25–15:00, 16:05–20:15");
  });

  it("reads now for an open span", () => {
    expect(spansText([{ start: at("09:00"), end: null }])).toBe("09:00–now");
  });
});
