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
const override = (patch: Partial<AppOverride> = {}): AppOverride => ({ hidden: false, ownLane: false, ...patch });
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
        ["Join short gaps", "16 min", { continuity: "extended" }],
        ["Join short gaps", "Off", { continuity: "off" }],
        ["Hide activity shorter than", "6 min", { noise: "high" }],
        ["Hide activity shorter than", "Off", { noise: "off" }],
        ["Titles", "App", { titles: "app" }],
        ["Time snapping (minutes)", "15", { snapMinutes: 15 }],
      ] as const)("%s: choosing %s saves it", async (name, option, patch) => {
        renderPage();
        await userEvent.click(group(name).getByRole("button", { name: option }));
        expect(saved()).toEqual(settings({ timeline: timelineWith(patch) }));
      });

      it.each([
        ["Join short gaps", ["Off", "2 min", "8 min", "16 min"]],
        ["Hide activity shorter than", ["Off", "2 min", "4 min", "6 min"]],
      ])("offers %s as %j", (name, options) => {
        renderPage();
        expect(group(name).getAllByRole("button").map((b) => b.textContent)).toEqual(options);
      });

      it("presses the current choice", () => {
        renderPage({ timeline: timelineWith({ continuity: "regular", noise: "medium" }) });
        expect(group("Join short gaps").getByRole("button", { name: "8 min" })).toHaveAttribute("aria-pressed", "true");
        expect(group("Join short gaps").getByRole("button", { name: "2 min" })).toHaveAttribute("aria-pressed", "false");
        expect(group("Hide activity shorter than").getByRole("button", { name: "4 min" })).toHaveAttribute("aria-pressed", "true");
      });

      it.each(["Continuity", "Noise cancellation"])("no longer has a %s control", (name) => {
        renderPage();
        expect(screen.queryByRole("group", { name })).not.toBeInTheDocument();
      });
    });
  });

  describe("apps", () => {
    it("lists each app with its name and tracked time", async () => {
      renderPage();
      expect(await screen.findByText("Safari")).toBeInTheDocument();
      expect(screen.getByText("1h 00m tracked")).toBeInTheDocument();
    });

    it.each([
      [3_600_000, "1h 00m tracked"],
      [30_000, "Under a minute tracked"],
      [0, "Not tracked yet"],
    ])("describes an app with %s ms as %s", async (totalMs, description) => {
      mocked.appsList.mockResolvedValue([knownApp({ totalMs })]);
      renderPage();
      expect(await screen.findByText(description)).toBeInTheDocument();
    });

    it("never shows the bundle id of an app", async () => {
      renderPage();
      await screen.findByText("Safari");
      expect(screen.queryByText(SAFARI)).not.toBeInTheDocument();
    });

    it("offers only the Hidden and Own lane switches for an app", async () => {
      mocked.appsList.mockResolvedValue([knownApp(), knownApp({ appKey: "com.apple.Notes", app: "Notes" })]);
      renderPage();
      await screen.findByText("Notes");
      expect(screen.getAllByRole("switch", { name: "Hidden" })).toHaveLength(2);
      expect(screen.getAllByRole("switch", { name: "Own lane" })).toHaveLength(2);
      expect(screen.queryByRole("group", { name: "Group Safari by app" })).not.toBeInTheDocument();
      expect(screen.queryByRole("group", { name: "Titles of Safari" })).not.toBeInTheDocument();
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

    it("checks Own lane for an app whose override has it on", async () => {
      renderPage({ apps: { [SAFARI]: override({ ownLane: true }) } });
      await screen.findByText("Safari");
      expect(screen.getByRole("switch", { name: "Own lane" })).toBeChecked();
      expect(screen.getByRole("switch", { name: "Hidden" })).not.toBeChecked();
    });
  });
});
