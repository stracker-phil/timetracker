import { describe, expect, it } from "vitest";

const sources = import.meta.glob<string>(
  ["/src/**/*.{ts,tsx}", "!/src/**/*.test.*", "!/src/test/**", "!/src/test-setup.ts", "!/src/**/*.gen.ts", "!/src/**/*.d.ts"],
  { query: "?raw", import: "default", eager: true },
);

const IMPORT_SPECIFIER = /(?:\bfrom\s+|\bimport\s*\(\s*|^\s*import\s+)["']([^"']+)["']/gm;

function specifiers(source: string): string[] {
  return [...source.matchAll(IMPORT_SPECIFIER)].map((m) => m[1]);
}

const files = Object.entries(sources);

describe("module boundaries", () => {
  it("finds the sources it checks", () => {
    expect(files.length).toBeGreaterThan(20);
    expect(Object.keys(sources)).toContain("/src/api/index.ts");
  });

  it("keeps Tauri imports in src/api/index.ts and src/<module>/api.ts", () => {
    const allowed = (file: string) => file === "/src/api/index.ts" || /^\/src\/[^/]+\/api\.ts$/.test(file);
    const offenders = files
      .filter(([file]) => !allowed(file))
      .filter(([, source]) => specifiers(source).some((s) => s.startsWith("@tauri-apps/api") || s.startsWith("@tauri-apps/plugin-")))
      .map(([file]) => file);
    expect(offenders).toEqual([]);
  });
});
