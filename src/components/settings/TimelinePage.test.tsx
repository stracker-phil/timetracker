import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type AppOverride, type KnownApp, type Settings } from "../../api";
import { installCommands } from "../../commands";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsDialog } from "./SettingsDialog";

const mocked = vi.mocked(api, true);
const override = (patch: Partial<AppOverride> = {}): AppOverride => ({ groupByApp: null, titles: null, hidden: false, ownLane: false, ...patch });
const knownApp = (patch: Partial<KnownApp> = {}): KnownApp => ({ appKey: "com.apple.Safari", app: "Safari", totalMs: 3_600_000, lastSeen: 1, ...patch });

const SAFARI = "com.apple.Safari";

function renderPage(patch: Partial<Settings> = {}) {
  resetStore({ settings: settings(patch), dialog: { type: "settings", page: "timeline" } });
  installCommands();
  return render(<SettingsDialog />);
}

const timelineWith = (patch: Partial<Settings["timeline"]>) => ({ ...settings().timeline, ...patch });
const saved = () => mocked.updateSettings.mock.lastCall?.[0];
const group = (name: string) => within(screen.getByRole("group", { name }));

describe("Settings Timeline page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.appsList.mockResolvedValue([knownApp()]);
  });

  describe("timeline controls", () => {
    it("saves the flipped Summary lane switch and nothing else", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("switch", { name: "Summary lane" }));
      expect(saved()).toEqual(settings({ timeline: timelineWith({ summaryLane: false }) }));    });

    it.each([
      ["Show all activity", "showAllActivity", false],
      ["Compact activity", "compact", true],
      ["Group by app and website", "groupByApp", false],
    ] as const)("saves the %s switch", async (name, field, expected) => {
      renderPage();
      await userEvent.click(screen.getByRole("switch", { name }));
      expect(saved()?.timeline[field]).toBe(expected);
    });

    it("applies a change without a toast", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("switch", { name: "Compact activity" }));
      expect(screen.queryByText("Settings saved")).not.toBeInTheDocument();
    });

    describe("summary interval", () => {
      it("shows the current interval in minutes", () => {
        renderPage();
        expect(screen.getByText("5 min")).toBeInTheDocument();
      });

      it("saves the minutes of the chosen step", () => {
        renderPage();
        fireEvent.change(screen.getByLabelText("Summary interval"), { target: { value: "4" } });
        expect(saved()?.timeline.summaryMinutes).toBe(30);
      });

      it("is hidden while the summary lane is off", () => {
        renderPage({ timeline: timelineWith({ summaryLane: false }) });
        expect(screen.queryByLabelText("Summary interval")).not.toBeInTheDocument();
      });
    });

    describe("segmented choices", () => {
      it.each([
        ["Continuity", "Extended", { continuity: "extended" }],
        ["Noise cancellation", "High", { noise: "high" }],
        ["Titles", "App", { titles: "app" }],
        ["Time snapping (minutes)", "15", { snapMinutes: 15 }],
      ] as const)("%s: choosing %s saves it", async (name, option, patch) => {
        renderPage();
        await userEvent.click(group(name).getByRole("button", { name: option }));
        expect(saved()).toEqual(settings({ timeline: timelineWith(patch) }));
      });

      it("presses the current choice", () => {
        renderPage({ timeline: timelineWith({ continuity: "regular" }) });
        expect(group("Continuity").getByRole("button", { name: "Regular" })).toHaveAttribute("aria-pressed", "true");
        expect(group("Continuity").getByRole("button", { name: "Short" })).toHaveAttribute("aria-pressed", "false");
      });
    });
  });

  describe("apps", () => {
    it("lists each app with its name and tracked time", async () => {
      renderPage();
      expect(await screen.findByText("Safari")).toBeInTheDocument();
      expect(screen.getByText("1:00")).toBeInTheDocument();
    });

    it("shows the key of an app without tracked time", async () => {
      mocked.appsList.mockResolvedValue([knownApp({ appKey: "com.example.new", app: "New", totalMs: 0 })]);
      renderPage();
      expect(await screen.findByText("com.example.new")).toBeInTheDocument();
    });

    it("says no apps are tracked when the list is empty", async () => {
      mocked.appsList.mockResolvedValue([]);
      renderPage();
      expect(await screen.findByText("No apps tracked yet")).toBeInTheDocument();
    });

    it("shows the error when the apps cannot be listed", async () => {
      mocked.appsList.mockRejectedValue({ kind: "db", message: "Database locked", code: null });
      renderPage();
      expect(await screen.findByText("Could not list the apps")).toBeInTheDocument();
      expect(screen.getByText("Database locked")).toBeInTheDocument();
    });

    it.each([
      ["Hidden", { hidden: true }],
      ["Own lane", { ownLane: true }],
    ] as const)("the %s switch sends it for the app", async (name, patch) => {
      renderPage();
      await screen.findByText("Safari");
      const [appSwitch] = screen.getAllByRole("switch", { name });
      await userEvent.click(appSwitch);
      expect(mocked.setAppSetting).toHaveBeenCalledWith(SAFARI, patch);
    });

    it("turns Hidden off again for a hidden app", async () => {
      renderPage({ apps: { [SAFARI]: override({ hidden: true }) } });
      await screen.findByText("Safari");
      const hidden = screen.getByRole("switch", { name: "Hidden" });
      expect(hidden).toBeChecked();
      await userEvent.click(hidden);
      expect(mocked.setAppSetting).toHaveBeenCalledWith(SAFARI, { hidden: false });
    });

    it.each([
      ["Off", { groupByApp: false }],
      ["On", { groupByApp: true }],
      ["Default", { groupByApp: null }],
    ] as const)("choosing %s for Group by app sends it", async (option, patch) => {
      renderPage({ apps: { [SAFARI]: override({ groupByApp: option === "Default" ? true : null }) } });
      await screen.findByText("Safari");
      await userEvent.click(group("Group Safari by app").getByRole("button", { name: option }));
      expect(mocked.setAppSetting).toHaveBeenCalledWith(SAFARI, patch);
    });

    it.each([
      ["Document", { titles: "document" }],
      ["App", { titles: "app" }],
      ["Default", { titles: null }],
    ] as const)("choosing %s for the titles of an app sends it", async (option, patch) => {
      renderPage({ apps: { [SAFARI]: override({ titles: option === "Default" ? "app" : null }) } });
      await screen.findByText("Safari");
      await userEvent.click(group("Titles of Safari").getByRole("button", { name: option }));
      expect(mocked.setAppSetting).toHaveBeenCalledWith(SAFARI, patch);
    });

    it("presses the choices stored in the app's override", async () => {
      renderPage({ apps: { [SAFARI]: override({ groupByApp: false, titles: "document", ownLane: true }) } });
      await screen.findByText("Safari");
      expect(group("Group Safari by app").getByRole("button", { name: "Off" })).toHaveAttribute("aria-pressed", "true");
      expect(group("Group Safari by app").getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "false");
      expect(group("Titles of Safari").getByRole("button", { name: "Document" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("switch", { name: "Own lane" })).toBeChecked();
    });

    it("presses Default for an app without an override", async () => {
      renderPage();
      await screen.findByText("Safari");
      expect(group("Group Safari by app").getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "true");
      expect(group("Titles of Safari").getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "true");
    });
  });
});
