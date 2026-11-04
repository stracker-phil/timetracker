import { describe, expect, it } from "vitest";
import { registerSettingsPage, resolveSettingsPage, settingsGroups, settingsPages } from "./settingsPages";

const ids = (query?: string) => settingsGroups(query).flatMap((g) => g.pages.map((entry) => entry.page.id));

describe("resolveSettingsPage", () => {
  it("returns the page with the given id", () => {
    const resolved = resolveSettingsPage("capture");
    expect(resolved?.page.id).toBe("capture");
    expect(resolved?.section).toBeUndefined();
  });

  it("returns the page that holds the given section id, with that section", () => {
    const resolved = resolveSettingsPage("look");
    expect(resolved?.page.id).toBe("appearance");
    expect(resolved?.section).toBe("look");
  });

  it.each([undefined, "no-such-page"])("falls back to the first page, without a section, for %s", (name) => {
    const resolved = resolveSettingsPage(name);
    expect(resolved?.page.id).toBe(settingsPages()[0].id);
    expect(resolved?.section).toBeUndefined();
  });
});

describe("settingsGroups", () => {
  it("lists every page in nav order for an empty query", () => {
    expect(settingsGroups().map((g) => g.group)).toEqual(["App", "Tracking", "System"]);
    expect(ids("")).toEqual(["general", "appearance", "shortcuts", "capture", "tracker", "timeline", "data"]);
  });

  it("finds a page by a section keyword and names the section", () => {
    const [group] = settingsGroups("accent");
    expect(group.group).toBe("App");
    expect(group.pages.map((entry) => [entry.page.id, entry.section?.title])).toEqual([["appearance", "Look"]]);
  });

  it("finds a page by its label without naming a section", () => {
    const entries = settingsGroups("tracker").flatMap((g) => g.pages);
    expect(entries.find((entry) => entry.page.id === "tracker")).toEqual({ page: resolveSettingsPage("tracker")!.page });
  });

  it("finds every page of a group by the group's name", () => {
    expect(ids("tracking")).toEqual(["capture", "tracker", "timeline"]);
  });

  it("ignores case and surrounding spaces", () => {
    expect(ids("  ACCENT ")).toEqual(["appearance"]);
  });

  it("is empty when nothing matches", () => {
    expect(settingsGroups("zzz")).toEqual([]);
  });

  it("puts a module's page in an unknown group after the known groups", () => {
    registerSettingsPage({ id: "zeta-page", group: "Zeta", label: "Zeta", icon: (() => null) as never, Body: () => null });
    expect(settingsGroups().map((g) => g.group)).toEqual(["App", "Tracking", "System", "Zeta"]);
  });
});
