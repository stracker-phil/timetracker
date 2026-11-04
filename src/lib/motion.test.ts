import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { type MotionToken, motionMs, useFlash, usePresence } from "./motion";

const TOKENS: MotionToken[] = ["--motion-fast", "--motion-base", "--motion-slow"];

function setToken(token: MotionToken, value: string) {
  document.documentElement.style.setProperty(token, value);
}

afterEach(() => {
  for (const token of TOKENS) document.documentElement.style.removeProperty(token);
  vi.useRealTimers();
});

describe("motionMs", () => {
  it.each([
    ["150ms", 150],
    ["0.2s", 200],
    ["1s", 1000],
    ["0ms", 0],
    ["fast", 0],
    ["", 0],
  ])("reads %j as %d ms", (value, expected) => {
    setToken("--motion-base", value);
    expect(motionMs("--motion-base")).toBe(expected);
  });

  it("is 0 when the token is not set", () => {
    expect(motionMs("--motion-slow")).toBe(0);
  });
});

describe("usePresence", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    setToken("--motion-base", "150ms");
  });

  it("is mounted and open while shown", () => {
    const { result } = renderHook(() => usePresence(true));
    expect(result.current).toEqual({ mounted: true, state: "open" });
  });

  it("is not mounted when it starts hidden", () => {
    const { result } = renderHook(() => usePresence(false));
    expect(result.current.mounted).toBe(false);
  });

  it("stays mounted as leaving until the token's time has passed", async () => {
    const { result, rerender } = renderHook(({ shown }) => usePresence(shown), { initialProps: { shown: true } });
    rerender({ shown: false });
    expect(result.current).toEqual({ mounted: true, state: "leaving" });

    await act(() => vi.advanceTimersByTimeAsync(100));
    expect(result.current.mounted).toBe(true);

    await act(() => vi.advanceTimersByTimeAsync(60));
    expect(result.current.mounted).toBe(false);
  });

  it("waits for the given token instead of the base one", async () => {
    setToken("--motion-slow", "400ms");
    const { result, rerender } = renderHook(({ shown }) => usePresence(shown, "--motion-slow"), { initialProps: { shown: true } });
    rerender({ shown: false });

    await act(() => vi.advanceTimersByTimeAsync(200));
    expect(result.current.mounted).toBe(true);

    await act(() => vi.advanceTimersByTimeAsync(250));
    expect(result.current.mounted).toBe(false);
  });

  it("cancels the exit when shown again while leaving", async () => {
    const { result, rerender } = renderHook(({ shown }) => usePresence(shown), { initialProps: { shown: true } });
    rerender({ shown: false });
    await act(() => vi.advanceTimersByTimeAsync(100));
    rerender({ shown: true });
    expect(result.current).toEqual({ mounted: true, state: "open" });

    await act(() => vi.advanceTimersByTimeAsync(500));
    expect(result.current).toEqual({ mounted: true, state: "open" });
  });

  it("unmounts at once when the token is 0", () => {
    setToken("--motion-base", "0ms");
    const { result, rerender } = renderHook(({ shown }) => usePresence(shown), { initialProps: { shown: true } });
    rerender({ shown: false });
    expect(result.current.mounted).toBe(false);
  });
});

describe("useFlash", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  it("is false on mount", () => {
    const { result } = renderHook(({ value }) => useFlash(value), { initialProps: { value: "a" } });
    expect(result.current).toBe(false);
  });

  it("is true right after the value changes and false again after 1200ms", async () => {
    const { result, rerender } = renderHook(({ value }) => useFlash(value), { initialProps: { value: "a" } });
    rerender({ value: "b" });
    expect(result.current).toBe(true);

    await act(() => vi.advanceTimersByTimeAsync(1100));
    expect(result.current).toBe(true);

    await act(() => vi.advanceTimersByTimeAsync(150));
    expect(result.current).toBe(false);
  });

  it("restarts the timer when the value changes again", async () => {
    const { result, rerender } = renderHook(({ value }) => useFlash(value), { initialProps: { value: "a" } });
    rerender({ value: "b" });
    await act(() => vi.advanceTimersByTimeAsync(1000));
    rerender({ value: "c" });

    await act(() => vi.advanceTimersByTimeAsync(1000));
    expect(result.current).toBe(true);

    await act(() => vi.advanceTimersByTimeAsync(250));
    expect(result.current).toBe(false);
  });

  it("stays false when re-rendered with the same value", async () => {
    const { result, rerender } = renderHook(({ value }) => useFlash(value), { initialProps: { value: "a" } });
    rerender({ value: "a" });
    expect(result.current).toBe(false);
  });
});
