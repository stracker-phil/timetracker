import { afterEach, describe, expect, it, vi } from "vitest";
import { bindingFor, bindingFromEvent, bindingLabel, clearCommands, conflicts, registerCommands, registerScope, resolveKeydown, setOverrides, type Command, type KeyLike } from "./index";

const key = (key: string, mods: Partial<{ meta: boolean; ctrl: boolean; alt: boolean; shift: boolean }> = {}, target: EventTarget | null = null) => ({
  key,
  metaKey: mods.meta ?? false,
  ctrlKey: mods.ctrl ?? false,
  altKey: mods.alt ?? false,
  shiftKey: mods.shift ?? false,
  target,
});

function cmd(id: string, scope: Command["scope"], defaultBinding?: string, extra: Partial<Command> = {}): Command {
  return { id, title: id, scope, defaultBinding, run: vi.fn(), ...extra };
}

describe("bindings", () => {
  const normalized = (binding: string) => bindingFor("unregistered", { unregistered: binding });

  it.each([
    ["modifier order and key case", "shift+mod+k", "Mod+Shift+K"],
    ["the Cmd alias", "Cmd+[", "Mod+["],
    ["Ctrl with a named key", "Ctrl+Tab", "Ctrl+Tab"],
    ["a minus key", "mod+-", "Mod+-"],
    ["a trailing ++ as the plus key", "Mod++", "Mod++"],
    ["a trailing ++ with several modifiers", "shift+mod++", "Mod+Shift++"],
    ["a modifier without a key", "Mod+", null],
  ])("normalizes %s", (_name, input, expected) => {
    expect(normalized(input)).toBe(expected);
  });

  it("returns an override that binds the plus key", () => {
    registerCommands([cmd("x", "global")]);
    setOverrides({ x: "Mod+Shift++" });
    expect(bindingFor("x")).toBe("Mod+Shift++");
    clearCommands();
  });
});

describe("bindingFromEvent", () => {
  it("reads bindings from keyboard events", () => {
    expect(bindingFromEvent(key("k", { meta: true, shift: true }))).toBe("Mod+Shift+K");
    expect(bindingFromEvent(key("Tab", { ctrl: true }))).toBe("Ctrl+Tab");
  });

  it("ignores a bare modifier press", () => {
    expect(bindingFromEvent(key("Meta", { meta: true }))).toBeNull();
  });

  describe("physical keys", () => {
    const event = (patch: Partial<KeyLike>): KeyLike => ({ key: "", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...patch });

    it.each([
      ["a composed ⌥ character", { key: "˚", code: "KeyK", metaKey: true, altKey: true }, "Mod+Alt+K"],
      ["a dead key", { key: "Dead", code: "KeyN", altKey: true }, "Alt+N"],
      ["a non-Latin layout character", { key: "е", code: "KeyT", metaKey: true }, "Mod+T"],
      ["punctuation, which follows the layout", { key: "-", code: "Slash", metaKey: true }, "Mod+-"],
      ["a named key with ⌥", { key: "Enter", code: "Enter", altKey: true, metaKey: true }, "Mod+Alt+Enter"],
    ])("reads %s", (_name, patch, expected) => {
      expect(bindingFromEvent(event(patch))).toBe(expected);
    });
  });
});

describe("bindingLabel", () => {
  it.each([
    ["Mod+Shift+K", "⌘⇧K"],
    ["Ctrl+Tab", "⌃⇥"],
    ["Mod+Enter", "⌘↵"],
    ["Mod+Shift++", "⌘⇧+"],
    [null, ""],
  ])("labels %s as %s", (binding, expected) => {
    expect(bindingLabel(binding)).toBe(expected);
  });
});

describe("resolveKeydown", () => {
  afterEach(clearCommands);

  it("prefers the most specific active scope", () => {
    registerScope({ id: "alpha", parent: "global", label: "Alpha", isActive: () => true });
    registerCommands([cmd("g", "global", "Mod+F"), cmd("w", "alpha", "Mod+F")]);
    expect(resolveKeydown(key("f", { meta: true }), ["global", "alpha"])?.id).toBe("w");
    expect(resolveKeydown(key("f", { meta: true }), ["global"])?.id).toBe("g");
  });

  it("skips disabled commands and inactive scopes", () => {
    registerScope({ id: "alpha", parent: "global", label: "Alpha", isActive: () => true });
    registerCommands([cmd("off", "alpha", "Mod+R", { when: () => false }), cmd("on", "global", "Mod+R")]);
    expect(resolveKeydown(key("r", { meta: true }), ["global", "alpha"])?.id).toBe("on");
    expect(resolveKeydown(key("x", { meta: true }), ["global", "alpha"])).toBeNull();
  });

  it("fires only Mod and Ctrl combos (and Escape) while typing in an input", () => {
    registerCommands([cmd("plain", "global", "J"), cmd("mod", "global", "Mod+J"), cmd("ctrl", "global", "Ctrl+J"), cmd("esc", "global", "Escape")]);
    const input = document.createElement("input");
    expect(resolveKeydown(key("j", {}, input), ["global"])).toBeNull();
    expect(resolveKeydown(key("j", { meta: true }, input), ["global"])?.id).toBe("mod");
    expect(resolveKeydown(key("j", { ctrl: true }, input), ["global"])?.id).toBe("ctrl");
    expect(resolveKeydown(key("Escape", {}, input), ["global"])?.id).toBe("esc");
    expect(resolveKeydown(key("j", {}, document.createElement("div")), ["global"])?.id).toBe("plain");
  });

  it("honors user overrides, including unbinding", () => {
    registerCommands([cmd("a", "global", "Mod+K"), cmd("b", "global")]);
    setOverrides({ a: null, b: "Mod+Shift+X" });
    expect(bindingFor("a")).toBeNull();
    expect(bindingFor("b")).toBe("Mod+Shift+X");
    expect(resolveKeydown(key("k", { meta: true }), ["global"])).toBeNull();
    expect(resolveKeydown(key("x", { meta: true, shift: true }), ["global"])?.id).toBe("b");
  });
});

describe("conflicts", () => {
  afterEach(clearCommands);

  it("reports shared bindings only in overlapping scopes", () => {
    registerScope({ id: "alpha", parent: "global", label: "Alpha", isActive: () => true });
    registerScope({ id: "beta", parent: "global", label: "Beta", isActive: () => true });
    registerCommands([cmd("first", "alpha", "Mod+F"), cmd("second", "beta", "Mod+F"), cmd("inner", "alpha", "Mod+E"), cmd("outer", "global", "Mod+E")]);
    expect(conflicts().map((c) => [c.a.id, c.b.id])).toEqual([["inner", "outer"]]);
    expect(conflicts({ first: "Mod+E" }).map((c) => [c.a.id, c.b.id])).toEqual([
      ["first", "inner"],
      ["first", "outer"],
      ["inner", "outer"],
    ]);
  });
});
