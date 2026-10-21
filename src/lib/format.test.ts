import { describe, expect, it } from "vitest";
import { NOW } from "../test/now";
import { formatBytes, formatClock, formatDay, formatDuration, formatHm, relativeTime } from "./format";

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

describe("formatHm", () => {
  it.each([
    ["zero", 0, "0:00"],
    ["seconds are dropped", 59_999, "0:00"],
    ["minutes zero-padded", 5 * MIN, "0:05"],
    ["hours and minutes", 6 * HOUR + 12 * MIN, "6:12"],
    ["more than a day", 25 * HOUR, "25:00"],
    ["negative counts as zero", -MIN, "0:00"],
  ])("formats %s", (_name, ms, expected) => {
    expect(formatHm(ms)).toBe(expected);
  });
});

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

describe("formatBytes", () => {
  it.each([
    [0, "0 B"],
    [512, "512 B"],
    [1024, "1.0 KB"],
    [1536, "1.5 KB"],
    [5 * 1024 * 1024, "5.0 MB"],
  ])("formats %d bytes as %s", (bytes, expected) => {
    expect(formatBytes(bytes)).toBe(expected);
  });
});

describe("formatDay", () => {
  it("shows the local date with zero-padded month and day", () => {
    expect(formatDay(new Date(2026, 0, 5, 12).getTime())).toBe("2026-01-05");
    expect(formatDay(new Date(2026, 11, 31, 23, 59).getTime())).toBe("2026-12-31");
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
