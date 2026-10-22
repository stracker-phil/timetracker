import { afterEach, describe, expect, it, vi } from "vitest";

const system = vi.hoisted(() => {
  const state = { dark: false };
  window.matchMedia = ((query: string) => ({
    get matches() {
      return state.dark;
    },
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }));
  return state;
});

import { applyAppearance, applyInitialLook, resolveLook, schemeLook } from "./theme";
import { settings } from "../test/settings";

type Scheme = "default" | "light" | "dark";

describe("schemeLook", () => {
  it.each<[Scheme, string, string]>([
    ["default", "light", "dark"],
    ["light", "light", "light"],
    ["dark", "dark", "dark"],
  ])("gives the %s scheme %s content and a %s sidebar", (scheme, content, sidebar) => {
    expect(schemeLook(scheme)).toEqual({ content, sidebar });
  });
});

describe("resolveLook", () => {
  it.each<[Scheme, Scheme, boolean, string, string]>([
    ["default", "dark", false, "light", "dark"],
    ["default", "dark", true, "dark", "dark"],
    ["light", "light", false, "light", "light"],
    ["light", "light", true, "light", "light"],
    ["dark", "default", false, "dark", "dark"],
    ["dark", "default", true, "light", "dark"],
    ["default", "default", false, "light", "dark"],
    ["default", "default", true, "light", "dark"],
    ["dark", "dark", false, "dark", "dark"],
    ["light", "dark", false, "light", "light"],
    ["light", "dark", true, "dark", "dark"],
    ["dark", "light", false, "dark", "dark"],
    ["dark", "light", true, "light", "light"],
  ])("light %s and dark %s (system dark: %s) give %s content and a %s sidebar", (lightScheme, darkScheme, systemDark, content, sidebar) => {
    expect(resolveLook({ lightScheme, darkScheme }, systemDark)).toEqual({ content, sidebar });
  });
});

describe("applyAppearance", () => {
  const root = document.documentElement;

  it("sets the tones of the light mode scheme while macOS is light", () => {
    applyAppearance(settings({ lightScheme: "default", darkScheme: "light" }), false);
    expect(root.dataset.content).toBe("light");
    expect(root.dataset.sidebar).toBe("dark");
  });

  it("sets the tones of the dark mode scheme while macOS is dark", () => {
    applyAppearance(settings({ lightScheme: "default", darkScheme: "light" }), true);
    expect(root.dataset.content).toBe("light");
    expect(root.dataset.sidebar).toBe("light");
  });

  it("makes everything dark when the light mode scheme is Dark", () => {
    applyAppearance(settings({ lightScheme: "dark" }), false);
    expect(root.dataset.content).toBe("dark");
    expect(root.dataset.sidebar).toBe("dark");
  });

  it("sets the accent", () => {
    applyAppearance(settings({ accent: "teal" }), false);
    expect(root.dataset.accent).toBe("teal");
  });
});

describe("applyInitialLook", () => {
  const root = document.documentElement;
  afterEach(() => {
    system.dark = false;
  });

  it("uses the Default scheme while macOS is light", () => {
    system.dark = false;
    delete root.dataset.content;
    delete root.dataset.sidebar;
    applyInitialLook();
    expect(root.dataset.content).toBe("light");
    expect(root.dataset.sidebar).toBe("dark");
  });

  it("uses the Dark scheme while macOS is dark", () => {
    system.dark = true;
    delete root.dataset.content;
    delete root.dataset.sidebar;
    applyInitialLook();
    expect(root.dataset.content).toBe("dark");
    expect(root.dataset.sidebar).toBe("dark");
  });
});
