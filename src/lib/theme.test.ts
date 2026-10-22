import { describe, expect, it } from "vitest";
import { applyAppearance, applyInitialLook, lightModeLook, resolveLook } from "./theme";
import { settings } from "../test/settings";

type Mode = "light" | "dark" | "system";
type Scheme = "default" | "light" | "dark";

describe("resolveLook", () => {
  it.each<[Mode, Scheme, boolean, string, string]>([
    ["light", "default", false, "light", "dark"],
    ["light", "light", false, "light", "light"],
    ["light", "dark", false, "dark", "dark"],
    ["dark", "default", false, "dark", "dark"],
    ["dark", "light", false, "dark", "dark"],
    ["dark", "dark", false, "dark", "dark"],
    ["system", "default", true, "dark", "dark"],
    ["system", "light", true, "dark", "dark"],
    ["system", "dark", true, "dark", "dark"],
    ["system", "default", false, "light", "dark"],
    ["system", "light", false, "light", "light"],
    ["system", "dark", false, "dark", "dark"],
  ])("mode %s with scheme %s (system dark: %s) gives %s content and a %s sidebar", (mode, scheme, systemDark, content, sidebar) => {
    expect(resolveLook({ mode, scheme }, systemDark)).toEqual({ content, sidebar });
  });
});

describe("lightModeLook", () => {
  it.each<[Scheme, string, string]>([
    ["default", "light", "dark"],
    ["light", "light", "light"],
    ["dark", "dark", "dark"],
  ])("gives the %s scheme %s content and a %s sidebar", (scheme, content, sidebar) => {
    expect(lightModeLook(scheme)).toEqual({ content, sidebar });
  });
});

describe("applyAppearance", () => {
  it("sets the content and sidebar tones on the root element", () => {
    applyAppearance(settings({ mode: "light", scheme: "default" }), false);
    expect(document.documentElement.dataset.content).toBe("light");
    expect(document.documentElement.dataset.sidebar).toBe("dark");

    applyAppearance(settings({ mode: "dark", scheme: "light" }), false);
    expect(document.documentElement.dataset.content).toBe("dark");
    expect(document.documentElement.dataset.sidebar).toBe("dark");
  });

  it("follows the system appearance in system mode", () => {
    applyAppearance(settings({ mode: "system", scheme: "light" }), true);
    expect(document.documentElement.dataset.content).toBe("dark");
  });
});

describe("applyInitialLook", () => {
  it("sets both tones before settings are loaded", () => {
    delete document.documentElement.dataset.content;
    delete document.documentElement.dataset.sidebar;
    applyInitialLook();
    expect(document.documentElement.dataset.content).toMatch(/^(light|dark)$/);
    expect(document.documentElement.dataset.sidebar).toBe("dark");
  });
});
