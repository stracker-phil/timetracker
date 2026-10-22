import { describe, expect, it } from "vitest";
import { SHADES } from "../api";
import { shadeLightness } from "./accent";

describe("shadeLightness", () => {
  it("gets lighter with every shade and stays within OKLCH lightness 0 to 1", () => {
    const values = Array.from({ length: SHADES }, (_, shade) => shadeLightness(shade));
    values.forEach((value, i) => {
      expect(value).toBeGreaterThan(0);
      expect(value).toBeLessThan(1);
      if (i > 0) expect(value).toBeGreaterThan(values[i - 1]);
    });
  });
});
