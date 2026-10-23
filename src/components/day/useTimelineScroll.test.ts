import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { at, block, dayBounds, dayView, lane } from "../../test/dayFixtures";
import { useTimelineScroll } from "./useTimelineScroll";

const VIEWPORT_PX = 600;
const PX_PER_MINUTE = 2.5;
const MINUTE_MS = 60_000;

const PAST = "2026-03-10";
const view = (day: string, startTime?: string) => dayView({ day, lanes: startTime ? [lane({ blocks: [block({ id: "a1", start: at(startTime, day), end: at("18:00", day) })] })] : [] });
const yOf = (day: string) => (ms: number) => ((ms - dayBounds(day)[0]) / MINUTE_MS) * PX_PER_MINUTE;

function scrollBox() {
  const el = document.createElement("div");
  Object.defineProperty(el, "clientHeight", { configurable: true, value: VIEWPORT_PX });
  return { current: el };
}

interface Props {
  day: string;
  now: number | null;
  openings: number;
  startTime?: string;
}

function renderScroll(initial: Props) {
  const ref = scrollBox();
  const hook = renderHook(({ day, now, openings, startTime }: Props) => useTimelineScroll(ref, view(day, startTime), null, yOf(day), now, openings), { initialProps: initial });
  return { box: ref.current, ...hook };
}

describe("useTimelineScroll", () => {
  it("opens today with now at 80% of the viewport height", () => {
    const now = at("14:00", PAST);
    const { box } = renderScroll({ day: PAST, now, openings: 0 });
    expect(box.scrollTop).toBe(yOf(PAST)(now) - VIEWPORT_PX * 0.8);
  });

  it("opens today at the top when now is too early to scroll that far", () => {
    const { box } = renderScroll({ day: PAST, now: at("00:30", PAST), openings: 0 });
    expect(box.scrollTop).toBe(0);
  });

  it("opens a past day half an hour before its first activity", () => {
    const { box } = renderScroll({ day: PAST, now: null, openings: 0, startTime: "10:00" });
    expect(box.scrollTop).toBe(yOf(PAST)(at("09:30", PAST)));
  });

  it("opens a past day without activity half an hour before 08:00", () => {
    const { box } = renderScroll({ day: PAST, now: null, openings: 0 });
    expect(box.scrollTop).toBe(yOf(PAST)(at("07:30", PAST)));
  });

  it("keeps the position when the same day reloads", () => {
    const { box, rerender } = renderScroll({ day: PAST, now: at("14:00", PAST), openings: 0 });
    box.scrollTop = 123;
    rerender({ day: PAST, now: at("14:05", PAST), openings: 0 });
    expect(box.scrollTop).toBe(123);
  });

  it("scrolls to the new day when the day changes", () => {
    const other = "2026-03-11";
    const { box, rerender } = renderScroll({ day: PAST, now: null, openings: 0, startTime: "10:00" });
    rerender({ day: other, now: null, openings: 0, startTime: "12:00" });
    expect(box.scrollTop).toBe(yOf(other)(at("11:30", other)));
  });

  it("scrolls back to now on the same day when the window opens again", () => {
    const now = at("14:00", PAST);
    const { box, rerender } = renderScroll({ day: PAST, now, openings: 0 });
    box.scrollTop = 5;
    rerender({ day: PAST, now, openings: 1 });
    expect(box.scrollTop).toBe(yOf(PAST)(now) - VIEWPORT_PX * 0.8);
  });
});
