import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { formatDayLabel, formatMonthLabel, formatRangeLabel, isToday, today } from "./day";

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

  describe("formatDayLabel", () => {
    it("shows weekday, month and day for this year", () => {
      vi.useFakeTimers({ toFake: ["Date"], now: new Date(2026, 9, 21, 12) });
      expect(formatDayLabel("2026-10-21")).toBe("Wed, Oct 21");
    });

    it("adds the year for another year", () => {
      vi.useFakeTimers({ toFake: ["Date"], now: new Date(2026, 9, 21, 12) });
      expect(formatDayLabel("2025-12-31")).toBe("Wed, Dec 31, 2025");
    });
  });

  describe("range labels", () => {
    beforeEach(() => void vi.useFakeTimers({ toFake: ["Date"], now: new Date(2026, 9, 21, 12) }));

    it.each([
      ["within a month", "2026-10-19", "2026-10-25", "Oct 19 – 25"],
      ["across months", "2026-10-26", "2026-11-01", "Oct 26 – Nov 1"],
      ["across years", "2025-12-29", "2026-01-04", "Dec 29, 2025 – Jan 4, 2026"],
      ["within a month of another year", "2025-03-03", "2025-03-09", "Mar 3 – 9, 2025"],
      ["of a single day", "2026-10-21", "2026-10-21", "Wed, Oct 21"],
    ])("formatRangeLabel shows a range %s", (_name, from, to, expected) => {
      expect(formatRangeLabel(from, to)).toBe(expected);
    });

    it.each([
      ["2026-10-21", "October 2026"],
      ["2027-01-31", "January 2027"],
    ])("formatMonthLabel of %s is %s", (day, expected) => {
      expect(formatMonthLabel(day)).toBe(expected);
    });
  });
});
