import { afterEach, describe, expect, it, vi } from "vitest";
import { formatMonthLabel, isToday, today } from "./day";

describe("day helpers", () => {
  afterEach(() => vi.useRealTimers());

  describe("today", () => {
    it("is the local calendar day of the clock", () => {
      vi.useFakeTimers({ toFake: ["Date"], now: new Date(2026, 9, 21, 23, 59) });
      expect(today()).toBe("2026-10-21");
      expect(isToday("2026-10-21")).toBe(true);
      expect(isToday("2026-10-20")).toBe(false);
    });
  });

  describe("formatMonthLabel", () => {
    it.each([
      ["2026-10-21", "October 2026"],
      ["2027-01-31", "January 2027"],
    ])("of %s is %s", (day, expected) => {
      expect(formatMonthLabel(day)).toBe(expected);
    });
  });
});
