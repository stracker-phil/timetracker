/// <reference types="node" />
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { ACCENTS, SHADES } from "../api";
import { blockColors, contrastRatio, hexToRgb, oklchToRgb, over, rgbToOklch, type BlockTokens, type Rgb } from "./contrast";

const read = (name: string) => readFileSync(resolve(process.cwd(), "src/styles/palettes", `${name}.css`), "utf8");

function tokens(name: string): Record<string, string> {
  const text = read(name).replace(/\/\*[\s\S]*?\*\//g, "");
  return Object.fromEntries([...text.matchAll(/^\s*(--[\w-]+)\s*:\s*([^;]+);/gm)].map((m) => [m[1], m[2].trim()]));
}

const rgb = (t: Record<string, string>, name: string): Rgb => {
  expect(t[name], name).toMatch(/^#[0-9a-f]{6}$/i);
  return hexToRgb(t[name]);
};

const blockTokens = (t: Record<string, string>): BlockTokens => ({
  fillL: Number(t["--block-fill-l"]),
  fillSpread: Number(t["--block-fill-spread"]),
  fillC: Number(t["--block-fill-c"]),
  textL: Number(t["--block-text-l"]),
  textC: Number(t["--block-text-c"]),
});

describe("contrast math", () => {
  it("is 21 for black on white and 1 for equal colors", () => {
    expect(contrastRatio([0, 0, 0], [1, 1, 1])).toBeCloseTo(21, 5);
    expect(contrastRatio([1, 1, 1], [0, 0, 0])).toBeCloseTo(21, 5);
    expect(contrastRatio([0.4, 0.5, 0.6], [0.4, 0.5, 0.6])).toBeCloseTo(1, 5);
  });

  it.each(["#b9530f", "#1f6fbf", "#f2857a", "#6b6863"])("round-trips %s through OKLCH", (hex) => {
    const original = hexToRgb(hex);
    const [L, C, H] = rgbToOklch(original);
    oklchToRgb(L, C, H).forEach((channel, i) => expect(channel).toBeCloseTo(original[i], 3));
  });

  it("puts white at OKLCH lightness 1 and black at 0", () => {
    expect(rgbToOklch([1, 1, 1])[0]).toBeCloseTo(1, 3);
    expect(rgbToOklch([0, 0, 0])[0]).toBeCloseTo(0, 3);
  });

  it("composites a color over a background by alpha", () => {
    expect(over([1, 1, 1], 0.5, [0, 0, 0])).toEqual([0.5, 0.5, 0.5]);
  });
});

describe.each(["content-light", "content-dark"])("%s palette", (name) => {
  const t = tokens(name);
  const bg = rgb(t, "--bg");
  const block = blockTokens(t);

  const blocks = ACCENTS.flatMap((accent) => Array.from({ length: SHADES }, (_, shade) => ({ accent, shade })));

  it("keeps block text at 4.5:1 on its fill for every accent and shade", () => {
    const failing = blocks
      .map(({ accent, shade }) => {
        const c = blockColors(t[`--a-${accent}`], shade, block);
        return { accent, shade, ratio: contrastRatio(c.text, c.fill) };
      })
      .filter((b) => b.ratio < 4.5);
    expect(failing).toEqual([]);
  });

  it("keeps block meta at 4.5:1 on its fill for every accent and shade", () => {
    const failing = blocks
      .map(({ accent, shade }) => {
        const c = blockColors(t[`--a-${accent}`], shade, block);
        return { accent, shade, ratio: contrastRatio(c.meta, c.fill) };
      })
      .filter((b) => b.ratio < 4.5);
    expect(failing).toEqual([]);
  });

  it.each(ACCENTS)("keeps --on-accent at 4.5:1 on the %s hue", (accent) => {
    expect(contrastRatio(rgb(t, "--on-accent"), rgb(t, `--a-${accent}`))).toBeGreaterThanOrEqual(4.5);
  });

  it("keeps secondary text (--o2) at 4.5:1 and tertiary text (--o3) at 3:1 on the background", () => {
    const text = rgb(t, "--text");
    expect(contrastRatio(over(text, 0.68, bg), bg)).toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(over(text, 0.52, bg), bg)).toBeGreaterThanOrEqual(3);
  });
});

describe.each([
  ["sidebar-light", "content-light"],
  ["sidebar-dark", "content-dark"],
])("%s palette", (name, content) => {
  const t = tokens(name);
  const hues = tokens(content);
  const bg = rgb(t, "--sidebar-bg");

  it.each(["--sidebar-text", "--sidebar-muted"])("keeps %s at 4.5:1 on the sidebar", (token) => {
    expect(contrastRatio(rgb(t, token), bg)).toBeGreaterThanOrEqual(4.5);
  });

  it.each(ACCENTS)("keeps its tone's %s hue at 3:1 on the sidebar", (accent) => {
    expect(contrastRatio(rgb(hues, `--a-${accent}`), bg)).toBeGreaterThanOrEqual(3);
  });
});
