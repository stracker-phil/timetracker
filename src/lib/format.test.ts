import { describe, expect, it } from "vitest";
import { NOW } from "../test/now";
import { formatClock, formatDuration, relativeTime } from "./format";

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

describe("formatDuration", () => {
  it.each([
    ["zero", 0, "0 min"],
    ["less than a minute", 59_999, "0 min"],
    ["whole minutes", 45 * MIN, "45 min"],
    ["an hour and some minutes, zero-padded", 65 * MIN, "1 h 05 min"],
    ["full hours", 10 * HOUR, "10 h 00 min"],
    ["a negative span", -5 * MIN, "0 min"],
  ])("formats %s", (_name, ms, expected) => {
    expect(formatDuration(ms)).toBe(expected);
  });
});

describe("formatClock", () => {
  it("shows local wall-clock time with zero padding", () => {
    expect(formatClock(new Date(2026, 9, 21, 9, 30).getTime())).toBe("09:30");
    expect(formatClock(new Date(2026, 9, 21, 17, 5).getTime())).toBe("17:05");
  });
});

describe("relativeTime", () => {
  it.each([
    ["a few seconds apart", NOW - 20_000, "just now"],
    ["minutes ago", NOW - 5 * MIN, "5 min ago"],
    ["hours ago", NOW - 2 * HOUR, "2 h ago"],
    ["days ago", NOW - 3 * DAY, "3 d ago"],
    ["months ago", NOW - 90 * DAY, "3 mo ago"],
    ["years ago", NOW - 2 * 365 * DAY, "2 y ago"],
    ["in the future", NOW + 3 * DAY, "in 3 d"],
  ])("describes a time %s", (_name, when, expected) => {
    expect(relativeTime(when, NOW)).toBe(expected);
  });

  it("accepts an ISO date", () => {
    expect(relativeTime(new Date(NOW - 2 * HOUR).toISOString(), NOW)).toBe("2 h ago");
  });
});
