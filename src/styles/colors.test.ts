/// <reference types="node" />
import { readdirSync, readFileSync } from "node:fs";
import { join, resolve, sep } from "node:path";
import { describe, expect, it } from "vitest";

// Read from disk: Vitest blanks every CSS module (also `?raw` ones) unless CSS processing is on.
const srcDir = resolve(process.cwd(), "src");
const palettesDir = join(srcDir, "styles", "palettes") + sep;

const all: [string, string][] = readdirSync(srcDir, { recursive: true, encoding: "utf8" })
  .filter((rel: string) => /\.(css|ts|tsx)$/.test(rel))
  .map((rel: string): [string, string] => [join(srcDir, rel), readFileSync(join(srcDir, rel), "utf8")]);

const paletteFiles = all.filter(([path]) => path.startsWith(palettesDir));
const scanned = all.filter(([path]) => !path.startsWith(palettesDir) && !/\.test\.tsx?$/.test(path) && !path.endsWith("types.gen.ts"));

const stripComments = (text: string) => text.replace(/\/\*[\s\S]*?\*\//g, "");

// CSS: any hex color. TS(X): only a hex inside a string literal, so "#3229" in prose or `#` in regexes pass.
const CSS_HEX = /#[0-9a-fA-F]{3,8}\b/g;
const TS_HEX = /["'`]#[0-9a-fA-F]{3,8}["'`]/g;
const FUNCTIONAL = /\b(?:rgba?|hsla?)\(/g;

function colorLiterals(path: string, source: string): string[] {
  const isCss = path.endsWith(".css");
  const text = isCss ? stripComments(source) : source;
  return [isCss ? CSS_HEX : TS_HEX, FUNCTIONAL].flatMap((re) => text.match(re) ?? []);
}

function customProperties(source: string): string[] {
  return [...stripComments(source).matchAll(/^\s*(--[\w-]+)\s*:/gm)].map((m) => m[1]).sort();
}

const palette = (name: string) => {
  const found = paletteFiles.find(([path]) => path.endsWith(`${sep}${name}.css`));
  if (!found) throw new Error(`missing palette ${name}`);
  return found[1];
};

describe("color literals", () => {
  it("live only in the palette files", () => {
    const offenders = scanned
      .map(([path, source]) => ({ path, found: colorLiterals(path, source) }))
      .filter((o) => o.found.length > 0)
      .map((o) => `${o.path}: ${o.found.join(", ")}`);
    expect(offenders).toEqual([]);
  });

  it("scan sees the stylesheets and flags a literal", () => {
    expect(scanned.filter(([path]) => path.endsWith(".css")).length).toBeGreaterThan(1);
    expect(scanned.length).toBeGreaterThan(10);
    expect(colorLiterals("a.css", "color: #fa8d3e;")).toHaveLength(1);
    expect(colorLiterals("a.ts", 'const c = "#fa8d3e";')).toHaveLength(1);
    expect(colorLiterals("a.css", "box-shadow: 0 0 4px rgba(0, 0, 0, 0.2)")).toHaveLength(1);
    expect(colorLiterals("a.css", "background: transparent; color: currentColor")).toEqual([]);
    expect(colorLiterals("a.ts", 'label: "Order #4711"')).toEqual([]);
  });
});

describe("palette files", () => {
  it("are exactly the two content and the two sidebar palettes", () => {
    const files = paletteFiles.map(([path]) => path.slice(palettesDir.length)).sort();
    expect(files).toEqual(["content-dark.css", "content-light.css", "sidebar-dark.css", "sidebar-light.css"]);
  });

  it("give both content palettes the same custom properties", () => {
    const expected = customProperties(palette("content-light"));
    expect(expected.length).toBeGreaterThan(0);
    expect(customProperties(palette("content-dark"))).toEqual(expected);
  });

  it("give both sidebar palettes the same custom properties", () => {
    const expected = customProperties(palette("sidebar-light"));
    expect(expected.length).toBeGreaterThan(0);
    expect(customProperties(palette("sidebar-dark"))).toEqual(expected);
  });

  it.each(["sidebar-light", "sidebar-dark"])("%s defines the sidebar surface and text tokens", (name) => {
    expect(customProperties(palette(name))).toEqual(
      expect.arrayContaining(["--sidebar-bg", "--sidebar-text", "--sidebar-muted", "--sidebar-hover", "--sidebar-selected", "--sidebar-line"]),
    );
  });
});
