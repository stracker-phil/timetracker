import { describe, expect, it } from "vitest";
import { settingsScore } from "./rank";

describe("settingsScore", () => {
  it.each([
    ["the title equals the needle", "Accent", "accent", 5],
    ["the title starts with the needle", "Accent", "acc", 4],
    ["the needle starts a word of the title", "Light mode scheme", "mode", 3],
    ["the needle is inside a word of the title", "Accent", "cent", 2],
    ["the needle differs in case", "Accent", "ACC", 4],
  ])("ranks the title when %s", (_case, title, needle, expected) => {
    expect(settingsScore(title, [], needle)).toBe(expected);
  });

  it.each([
    ["equals the needle", "tint"],
    ["starts with the needle", "tint colour"],
    ["has the needle at a word start", "a tint"],
  ])("caps a keyword that %s at an inside match", (_case, keyword) => {
    expect(settingsScore("Accent", [keyword], "tint")).toBe(2);
  });

  it("prefers the title's rank over a keyword's", () => {
    expect(settingsScore("Accent", ["accent colour"], "acc")).toBe(4);
  });

  it("scores the title's letters in order below a keyword match", () => {
    expect(settingsScore("Accent", [], "acnt")).toBe(1);
    expect(settingsScore("Accent", ["acnt"], "acnt")).toBe(2);
  });

  it("scores 0 when neither the title, a keyword nor the title's letters match", () => {
    expect(settingsScore("Accent", ["tint"], "zzz")).toBe(0);
  });

  it("does not match a keyword by its letters in order", () => {
    expect(settingsScore("Accent", ["tint"], "tnt")).toBe(0);
  });
});
