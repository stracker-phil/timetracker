import process from "node:process";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { addDays, addMonths, daysBetween, formatDayLabel, formatMonthLabel, formatRangeLabel, isToday, today } from "./day";

describe("day helpers", () => {
  afterEach(() => vi.useRealTimers());

  describe("addDays", () => {
    it.each([
      ["2026-10-21", 1, "2026-10-22"],
      ["2026-10-21", -1, "2026-10-20"],
      ["2026-02-28", 1, "2026-03-01"],
      ["2024-02-28", 1, "2024-02-29"],
      ["2026-12-31", 1, "2027-01-01"],
      ["2026-01-01", -1, "2025-12-31"],
      ["2026-10-21", 0, "2026-10-21"],
      ["2026-10-21", 30, "2026-11-20"],
    ])("%s plus %i is %s", (day, by, expected) => {
      expect(addDays(day, by)).toBe(expected);
    });

    describe.each(["America/New_York", "Europe/Berlin", "Australia/Sydney", "UTC"])("in %s", (zone) => {
      const original = process.env.TZ;
      const setZone = () => void (process.env.TZ = zone);
      const restore = () => void (original === undefined ? delete process.env.TZ : (process.env.TZ = original));

      it.each([
        ["2026-03-07", "2026-03-08", "2026-03-09"],
        ["2026-03-28", "2026-03-29", "2026-03-30"],
        ["2026-10-24", "2026-10-25", "2026-10-26"],
        ["2026-10-31", "2026-11-01", "2026-11-02"],
        ["2026-04-04", "2026-04-05", "2026-04-06"],
      ])("steps one day over the transition %s, %s, %s", (before, transition, after) => {
        setZone();
        try {
          expect(addDays(before, 1)).toBe(transition);
          expect(addDays(transition, 1)).toBe(after);
          expect(addDays(after, -1)).toBe(transition);
          expect(daysBetween(before, after)).toBe(2);
        } finally {
          restore();
        }
      });
    });
  });

  describe("addMonths", () => {
    it.each([
      ["2026-10-21", 1, "2026-11-01"],
      ["2026-10-21", -1, "2026-09-01"],
      ["2026-12-31", 1, "2027-01-01"],
      ["2026-01-15", -1, "2025-12-01"],
      ["2026-01-31", 1, "2026-02-01"],
      ["2026-03-31", -1, "2026-02-01"],
      ["2026-10-21", 0, "2026-10-01"],
      ["2026-10-21", 14, "2027-12-01"],
    ])("%s plus %i months is %s", (day, by, expected) => {
      expect(addMonths(day, by)).toBe(expected);
    });
  });

  describe("daysBetween", () => {
    it.each([
      ["2026-10-21", "2026-10-21", 0],
      ["2026-10-20", "2026-10-21", 1],
      ["2026-10-21", "2026-10-20", -1],
      ["2026-09-21", "2026-10-21", 30],
      ["2025-10-21", "2026-10-21", 365],
    ])("from %s to %s is %i", (from, to, expected) => {
      expect(daysBetween(from, to)).toBe(expected);
    });
  });

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
