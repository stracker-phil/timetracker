import { beforeEach, describe, expect, it } from "vitest";
import { resetStore, selectDayVersion, useStore } from "../store";

const version = (day: string) => selectDayVersion(day)(useStore.getState());

describe("data slice", () => {
  beforeEach(() => resetStore());

  it("changes only the version of the day that changed", () => {
    const before = { a: version("2026-10-20"), b: version("2026-10-21") };
    useStore.getState().dataChanged({ kind: "activity", day: "2026-10-20" });
    expect(version("2026-10-20")).not.toBe(before.a);
    expect(version("2026-10-21")).toBe(before.b);
  });

  it("changes the version again on every change of the same day", () => {
    useStore.getState().dataChanged({ kind: "activity", day: "2026-10-20" });
    const first = version("2026-10-20");
    useStore.getState().dataChanged({ kind: "activity", day: "2026-10-20" });
    expect(version("2026-10-20")).not.toBe(first);
  });

  it("changes the version of every day when no day is given", () => {
    const before = { a: version("2026-10-20"), b: version("2026-10-21") };
    useStore.getState().dataChanged({ kind: "activity", day: null });
    expect(version("2026-10-20")).not.toBe(before.a);
    expect(version("2026-10-21")).not.toBe(before.b);
  });
});
