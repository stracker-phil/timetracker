import { describe, expect, it } from "vitest";
import { toAppError } from "./index";

describe("toAppError", () => {
  it("passes an error of a known kind through", () => {
    const error = { kind: "overlap", message: "Overlaps another span", code: null, details: { start: 1 } };
    expect(toAppError(error)).toBe(error);
  });

  it.each([
    ["a string", "boom", "boom"],
    ["an Error", new Error("disk full"), "disk full"],
    ["an object with an unknown kind", { kind: "mystery", message: "odd" }, "odd"],
    ["an object without a message", { kind: "io" }, "[object Object]"],
    ["null", null, "null"],
  ])("wraps %s as an io error", (_name, input, message) => {
    expect(toAppError(input)).toEqual({ kind: "io", message, code: null });
  });
});
