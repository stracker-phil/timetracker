import { describe, expect, it } from "vitest";
import { registerSettingsPage, resolveSettingsPage, searchSettings, settingsGroups, settingsPages, type SettingsRow } from "./settingsPages";

const hits = (query: string) => searchSettings(query).map((match) => match.row?.id ?? match.page.id);
const pageIds = () => settingsGroups().flatMap((g) => g.pages.map((page) => page.id));
const icon = (() => null) as never;

describe("resolveSettingsPage", () => {
  it("returns the page with the given id and no row", () => {
    const resolved = resolveSettingsPage("capture");
    expect(resolved?.page.id).toBe("capture");
    expect(resolved?.row).toBeUndefined();
  });

  it("returns the page holding the given row id, with that row", () => {
    const resolved = resolveSettingsPage("accent");
    expect(resolved?.page.id).toBe("appearance");
    expect(resolved?.row?.title).toBe("Accent");
  });

  it("returns a row that depends on a switch with the anchor to reveal while it is hidden", () => {
    expect(resolveSettingsPage("summary-interval")?.row).toMatchObject({ title: "Summary interval", under: "summary-lane" });
  });

  it.each([undefined, "no-such-page"])("falls back to the first page, without a row, for %s", (name) => {
    const resolved = resolveSettingsPage(name);
    expect(resolved?.page.id).toBe(settingsPages()[0].id);
    expect(resolved?.row).toBeUndefined();
  });
});

describe("searchSettings", () => {
  it.each(["", "   "])("finds nothing for the blank query %j", (query) => {
    expect(searchSettings(query)).toEqual([]);
  });

  it("finds nothing when no page or row matches", () => {
    expect(searchSettings("qqqzzz")).toEqual([]);
  });

  it("finds a row and names the page it is on", () => {
    const [first] = searchSettings("accent");
    expect(first.page.id).toBe("appearance");
    expect(first.row?.title).toBe("Accent");
  });

  it("finds a page by its label, without a row", () => {
    const [first] = searchSettings("capture");
    expect(first.page.id).toBe("capture");
    expect(first.row).toBeUndefined();
  });

  it("ignores case and surrounding spaces", () => {
    expect(hits("  ACCENT ")[0]).toBe("accent");
  });

  it("finds a row by one of its keywords", () => {
    expect(hits("tint")).toContain("accent");
  });

  it("finds every page of a group by the group's name", () => {
    const pages = searchSettings("tracking")
      .filter((match) => !match.row)
      .map((match) => match.page.id);
    expect(pages).toEqual(["capture", "tracker", "timeline"]);
  });

  it("ranks a row whose title starts with the query above a keyword hit", () => {
    const found = hits("away");
    expect(found[0]).toBe("away-after");
    expect(found.indexOf("away-after")).toBeLessThan(found.indexOf("away-prompt-minutes"));
    expect(found.indexOf("away-prompt")).toBeLessThan(found.indexOf("away-prompt-minutes"));
  });

  it("finds a row by the letters of its title in order", () => {
    expect(hits("accnt")).toContain("accent");
  });

  it("lists a page before its rows when they match equally, in nav order", () => {
    const timeline = ["timeline", "summary-lane", "summary-interval", "show-all-activity"];
    expect(hits("lanes").filter((id) => timeline.includes(id))).toEqual(timeline);
  });
});

describe("settingsGroups", () => {
  it("lists every page in nav order under App, Tracking and System", () => {
    expect(settingsGroups().map((g) => g.group)).toEqual(["App", "Tracking", "System"]);
    expect(pageIds()).toEqual(["general", "appearance", "shortcuts", "capture", "tracker", "timeline", "data", "status", "about"]);
  });
});

describe("a page registered by a module", () => {
  let rows: SettingsRow[] = [{ id: "zebra-stripes", title: "Zebra stripes" }];

  it("puts a page in an unknown group after the known groups", () => {
    registerSettingsPage({ id: "zeta-page", group: "Zeta", label: "Zeta", icon, Body: () => null, rows: () => rows });
    expect(settingsGroups().map((g) => g.group)).toEqual(["App", "Tracking", "System", "Zeta"]);
  });

  it("is searched through rows given as a function, as they are when searching", () => {
    expect(searchSettings("zebra")).toMatchObject([{ page: { id: "zeta-page" }, row: { id: "zebra-stripes" } }]);
    rows = [{ id: "okapi-spots", title: "Okapi spots" }];
    expect(hits("zebra")).toEqual([]);
    expect(hits("okapi")).toEqual(["okapi-spots"]);
  });

  it("resolves a row id given by a function to its page", () => {
    expect(resolveSettingsPage("okapi-spots")).toMatchObject({ page: { id: "zeta-page" }, row: { title: "Okapi spots" } });
  });
});
