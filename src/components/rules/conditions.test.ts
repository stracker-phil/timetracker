import { describe, expect, it } from "vitest";
import { block } from "../../test/dayFixtures";
import { conditionSummary, draftFromBlock } from "./conditions";

const website = { id: "p-site", name: "Website" };

describe("conditionSummary", () => {
  it("joins the conditions with the field, the comparison and the quoted value", () => {
    expect(
      conditionSummary([
        { field: "app", op: "is", value: "Slack" },
        { field: "title", op: "contains", value: "standup" },
        { field: "url", op: "begins_with", value: "https://example.com" },
        { field: "path", op: "pattern", value: "*.pdf" },
      ]),
    ).toBe('app is "Slack" · title contains "standup" · url begins with "https://example.com" · path matches "*.pdf"');
  });

  it("is empty without conditions", () => {
    expect(conditionSummary([])).toBe("");
  });
});

describe("draftFromBlock", () => {
  it("names the rule after the app and the project and puts the project in", () => {
    const draft = draftFromBlock(block({ app: "Notes", title: "" }), website);
    expect(draft).toMatchObject({ name: "Notes → Website", projectId: "p-site", enabled: true, note: "", tagId: null });
  });

  it("names the rule after the app and leaves the project empty without one", () => {
    const draft = draftFromBlock(block({ app: "Notes", title: "" }), null);
    expect(draft).toMatchObject({ name: "Notes", projectId: "", enabled: true });
  });

  it("matches the app and a title that contains the block's title", () => {
    expect(draftFromBlock(block({ app: "Notes", title: "Todo" }), website).conditions).toEqual([
      { field: "app", op: "is", value: "Notes" },
      { field: "title", op: "contains", value: "Todo" },
    ]);
  });

  it("matches the domain of a URL block, without www., instead of the title", () => {
    expect(draftFromBlock(block({ app: "Safari", title: "Docs", url: "https://www.Example.com/docs?q=1" }), website).conditions).toEqual([
      { field: "app", op: "is", value: "Safari" },
      { field: "domain", op: "is", value: "example.com" },
    ]);
  });

  it("falls back to the title when the URL has no host", () => {
    expect(draftFromBlock(block({ app: "Preview", title: "Report", url: "not a url" }), website).conditions).toEqual([
      { field: "app", op: "is", value: "Preview" },
      { field: "title", op: "contains", value: "Report" },
    ]);
  });

  it("matches only the app when the block has no title", () => {
    expect(draftFromBlock(block({ app: "Notes", title: "" }), website).conditions).toEqual([{ field: "app", op: "is", value: "Notes" }]);
  });

  it("leaves a private block's title out", () => {
    expect(draftFromBlock(block({ app: "Safari", title: "Private", private: true }), website).conditions).toEqual([{ field: "app", op: "is", value: "Safari" }]);
  });
});
