import { describe, expect, it } from "vitest";
import { fuzzyScore, parsePrefix, score } from "./parse";

describe("parsePrefix", () => {
  it.each([
    [">x", ">", "x"],
    ["p: a", "p:", "a"],
    ["c:b", "c:", "b"],
    ["@ mon", "@", "mon"],
    ["  >x", ">", "x"],
    [" foo ", null, "foo"],
    ["", null, ""],
    [">", ">", ""],
  ])("splits %j into prefix %j and text %j", (input, prefix, needle) => {
    expect(parsePrefix(input)).toEqual({ prefix, needle });
  });
});

describe("score", () => {
  it("ranks equal over prefix over word start over inside over none", () => {
    const ranks = ["Reports", "Report card", "Copy report as text", "Unreported", "Zebra"].map((field) => score([field], "report"));
    expect(ranks).toEqual([4, 4, 3, 2, 0]);
    expect(score(["Report"], "report")).toBe(5);
    expect(score(["Reports"], "rep")).toBe(4);
    expect(score(["Copy report as text"], "rep")).toBe(3);
    expect(score(["Prepare"], "rep")).toBe(2);
  });

  it("ignores case", () => {
    expect(score(["REPORTS"], "rep")).toBe(4);
  });

  it("takes the best of several fields", () => {
    expect(score(["Prepare", "Reports"], "rep")).toBe(4);
  });

  it("matches everything with 1 when the text is empty", () => {
    expect(score(["Anything"], "")).toBe(1);
  });
});

describe("fuzzyScore", () => {
  it("scores like score when the text is contained", () => {
    expect(fuzzyScore(["Website"], "web")).toBe(4);
  });

  it("gives 1 when the characters occur in order", () => {
    expect(fuzzyScore(["Website"], "wbs")).toBe(1);
  });

  it("gives 0 when the characters are out of order", () => {
    expect(fuzzyScore(["Website"], "sbw")).toBe(0);
  });
});
