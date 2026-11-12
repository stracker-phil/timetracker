import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bind, clearCommands, holderOf, isDefaultBinding, reassign, registerCommands, registerScope, type Command } from "./index";

const cmd = (id: string, scope: Command["scope"], defaultBinding?: string): Command => ({ id, title: id, scope, defaultBinding, run: vi.fn() });

describe("shortcut edits", () => {
  beforeEach(() => {
    registerScope({ id: "alpha", parent: "global", label: "Alpha", isActive: () => true });
    registerScope({ id: "beta", parent: "global", label: "Beta", isActive: () => true });
    registerCommands([cmd("first", "alpha", "Mod+F"), cmd("second", "beta", "Mod+F"), cmd("inner", "alpha", "Mod+E"), cmd("outer", "global", "Mod+E"), cmd("free", "global")]);
  });
  afterEach(clearCommands);

  describe("isDefaultBinding", () => {
    it.each([
      ["has no override", {}, "first", true],
      ["is overridden with its default key", { first: "mod+f" }, "first", true],
      ["is overridden with another key", { first: "Mod+G" }, "first", false],
      ["has its default key removed", { first: null }, "first", false],
      ["has no default and no key", {}, "free", true],
      ["has no default and got a key", { free: "Mod+Z" }, "free", false],
    ])("tells whether a command that %s is on its default", (_name, custom, id, expected) => {
      expect(isDefaultBinding(id, custom)).toBe(expected);
    });
  });

  describe("holderOf", () => {
    it("finds the command that holds the key in the same scope", () => {
      expect(holderOf("inner", "Mod+F", {})?.id).toBe("first");
    });

    it("finds a holder in an enclosing scope", () => {
      expect(holderOf("inner", "Mod+E", {})?.id).toBe("outer");
    });

    it("finds a holder in a scope inside the command's own", () => {
      expect(holderOf("outer", "Mod+F", {})?.id).toBe("first");
    });

    it("never names the command itself", () => {
      expect(holderOf("outer", "Mod+E", {})?.id).toBe("inner");
      expect(holderOf("free", "Mod+Z", {})).toBeUndefined();
    });

    it("ignores a holder in a sibling scope", () => {
      expect(holderOf("inner", "Mod+F", { first: null })).toBeUndefined();
    });

    it("compares the key as the user's overrides have it", () => {
      expect(holderOf("inner", "Mod+Z", { free: "shift+mod+z" })).toBeUndefined();
      expect(holderOf("inner", "Mod+Shift+Z", { free: "shift+mod+z" })?.id).toBe("free");
    });

    it("does not count a key the holder gave up", () => {
      expect(holderOf("inner", "Mod+F", { first: "Mod+G" })).toBeUndefined();
    });

    it("is undefined for a free key and for an unknown command", () => {
      expect(holderOf("inner", "Mod+Q", {})).toBeUndefined();
      expect(holderOf("missing", "Mod+F", {})).toBeUndefined();
    });
  });

  describe("bind", () => {
    it("adds an override for a new key", () => {
      expect(bind({}, "first", "Mod+G")).toEqual({ first: "Mod+G" });
    });

    it("keeps the other overrides and does not change its input", () => {
      const custom = { inner: "Mod+H" };
      expect(bind(custom, "first", "Mod+G")).toEqual({ inner: "Mod+H", first: "Mod+G" });
      expect(custom).toEqual({ inner: "Mod+H" });
    });

    it("drops the override when the key is the default", () => {
      expect(bind({ first: "Mod+G", inner: "Mod+H" }, "first", "Mod+F")).toEqual({ inner: "Mod+H" });
    });

    it("stores null to unbind a command that has a default", () => {
      expect(bind({}, "first", null)).toEqual({ first: null });
    });

    it("drops the override when unbinding a command that has no default", () => {
      expect(bind({ free: "Mod+Z" }, "free", null)).toEqual({});
    });
  });

  describe("reassign", () => {
    it("gives the key to the command and unbinds the holder", () => {
      expect(reassign({}, "free", "Mod+E", "outer")).toEqual({ free: "Mod+E", outer: null });
    });

    it("drops the holder's override when it had no default key", () => {
      expect(reassign({ free: "Mod+Z" }, "outer", "Mod+Z", "free")).toEqual({ outer: "Mod+Z" });
    });

    it("keeps unrelated overrides", () => {
      expect(reassign({ second: "Mod+J" }, "free", "Mod+E", "outer")).toEqual({ second: "Mod+J", free: "Mod+E", outer: null });
    });
  });
});
