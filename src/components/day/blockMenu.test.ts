import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ActivityBlock, type AppOverride, type Lane, type Settings } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { appLane, block, lane, sourceLane } from "../../test/dayFixtures";
import { seedQuery, SETTINGS_KEY } from "../../test/queries";
import { settings } from "../../test/settings";
import type { MenuEntry } from "../menu";
import { blockMenu } from "./blockMenu";

const mocked = vi.mocked(api, true);
const KEY = "com.apple.Safari";

const override = (patch: Partial<AppOverride> = {}): AppOverride => ({ hidden: false, ownLane: false, ...patch });
const withTimeline = (timeline: Partial<Settings["timeline"]>, apps: Settings["apps"] = {}) => settings({ timeline: { ...settings().timeline, ...timeline }, apps });

type Item = Exclude<MenuEntry, "separator">;
const items = (entries: MenuEntry[]) => entries.filter((e): e is Item => e !== "separator");
const labels = (entries: MenuEntry[]) => entries.map((e) => (e === "separator" ? "---" : "caption" in e ? e.caption : e.text));
const entry = (entries: MenuEntry[], text: string) => {
  const found = items(entries).find((e) => "text" in e && e.text === text);
  if (!found || !("text" in found) || !found.run) throw new Error(`No menu entry "${text}"`);
  return { checked: found.checked, run: found.run };
};

function menu(s: Settings | null, b: ActivityBlock = block(), l: Lane = lane({ blocks: [b] })) {
  return blockMenu(b, l, s);
}

describe("blockMenu", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    seedQuery(SETTINGS_KEY, settings());
  });

  describe("of a tracker block", () => {
    it("lists the app settings, a separator, the copy entries, a separator, then Make a rule", () => {
      const url = block({ url: "https://example.com" });
      expect(labels(menu(settings(), url))).toEqual(["Own lane", "Hide Safari", "---", "Copy title", "Copy URL", "---", "Make a rule…"]);
    });

    it("has a single separator before Make a rule when no copy entry follows the app settings", () => {
      expect(labels(menu(settings(), block({ private: true })))).toEqual(["Own lane", "Hide Safari", "---", "Make a rule…"]);
    });

    it("has no Copy title for a private block but keeps Copy URL", () => {
      const secret = block({ private: true, url: "https://example.com" });
      expect(labels(menu(settings(), secret))).not.toContain("Copy title");
      expect(labels(menu(settings(), secret))).toContain("Copy URL");
    });

    it("offers only the copy entries and Make a rule while the settings are not loaded", () => {
      expect(labels(menu(null))).toEqual(["Copy title", "---", "Make a rule…"]);
    });
  });

  describe("of an external block", () => {
    it("offers only the copy entries and Make a rule", () => {
      const [external] = sourceLane("planner", [block({ url: "https://example.com" })]).blocks;
      expect(labels(menu(settings(), external))).toEqual(["Copy title", "Copy URL", "---", "Make a rule…"]);
    });

    it("offers Make a rule alone, without a leading separator, when nothing else applies", () => {
      const [external] = sourceLane("planner", [block({ private: true })]).blocks;
      expect(labels(menu(settings(), external))).toEqual(["Make a rule…"]);
    });
  });

  describe("Make a rule", () => {
    it("opens the rule sheet with the block's app as the condition and no project", () => {
      entry(menu(settings(), block({ app: "Notes", title: "" })), "Make a rule…").run();
      expect(useStore.getState().dialog).toMatchObject({
        type: "rule",
        draft: { name: "Notes", projectId: "", enabled: true, conditions: [{ field: "app", op: "is", value: "Notes" }] },
      });
    });

    it("adds the domain of a URL block to the conditions", () => {
      entry(menu(settings(), block({ app: "Safari", url: "https://www.example.com/docs" })), "Make a rule…").run();
      expect(useStore.getState().dialog).toMatchObject({
        draft: {
          conditions: [
            { field: "app", op: "is", value: "Safari" },
            { field: "domain", op: "is", value: "example.com" },
          ],
        },
      });
    });
  });

  describe("removed per-app entries", () => {
    it.each(["Group by app", "Titles: Document name", "Titles: App name"])("has no %s entry", (label) => {
      expect(labels(menu(settings(), block({ url: "https://example.com" })))).not.toContain(label);
    });
  });

  describe("Own lane", () => {
    it("is unchecked in the activity lane and moves the app out", () => {
      const m = menu(settings());
      expect(entry(m, "Own lane").checked).toBe(false);
      entry(m, "Own lane").run();
      expect(mocked.setAppSetting).toHaveBeenCalledWith(KEY, { ownLane: true });
    });

    it("is checked in an app lane and moves the app back", () => {
      const b = block();
      const m = menu(settings(), b, appLane("Safari", KEY, [b]));
      expect(entry(m, "Own lane").checked).toBe(true);
      entry(m, "Own lane").run();
      expect(mocked.setAppSetting).toHaveBeenCalledWith(KEY, { ownLane: false });
    });

    it("is checked by an override in another lane", () => {
      const m = menu(withTimeline({}, { [KEY]: override({ ownLane: true }) }));
      expect(entry(m, "Own lane").checked).toBe(true);
    });
  });

  describe("Hide", () => {
    it("hides the app and offers Undo, which shows it again", async () => {
      entry(menu(settings()), "Hide Safari").run();
      expect(mocked.setAppSetting).toHaveBeenLastCalledWith(KEY, { hidden: true });
      await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message)).toContain("Safari is hidden from the timeline"));
      useStore.getState().toasts.find((t) => t.message === "Safari is hidden from the timeline")?.action?.run();
      expect(mocked.setAppSetting).toHaveBeenLastCalledWith(KEY, { hidden: false });
    });

    it("shows no hidden toast when saving failed", async () => {
      mocked.setAppSetting.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      entry(menu(settings()), "Hide Safari").run();
      await vi.waitFor(() => expect(useStore.getState().toasts.length).toBeGreaterThan(0));
      expect(useStore.getState().toasts.map((t) => t.message)).not.toContain("Safari is hidden from the timeline");
    });
  });

  describe("Copy", () => {
    it("copies the title and the url", async () => {
      const writeText = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue(undefined);
      const m = menu(settings(), block({ title: "Docs", url: "https://example.com/docs" }));
      entry(m, "Copy title").run();
      await vi.waitFor(() => expect(writeText).toHaveBeenLastCalledWith("Docs"));
      entry(m, "Copy URL").run();
      await vi.waitFor(() => expect(writeText).toHaveBeenLastCalledWith("https://example.com/docs"));
    });
  });
});
