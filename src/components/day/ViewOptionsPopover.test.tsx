import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type DayView } from "../../api";
import { resetStore } from "../../state/store";
import { dayApp, dayView } from "../../test/dayFixtures";
import { settings } from "../../test/settings";
import { ViewOptionsPopover } from "./ViewOptionsPopover";

const mocked = vi.mocked(api, true);
const saved = () => mocked.updateSettings.mock.lastCall?.[0];
const mail = { appKey: "com.apple.Mail", app: "Mail" };
const notes = { appKey: "com.apple.Notes", app: "Notes" };

function renderPopover(view: Partial<DayView> = {}) {
  resetStore({ settings: settings(), dayView: dayView(view), dialog: { type: "viewOptions" } });
  return render(<ViewOptionsPopover />);
}

describe("ViewOptionsPopover", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("lanes and blocks", () => {
    it.each([
      ["Summary", "summaryLane", false],
      ["Activity", "showAllActivity", false],
      ["Compact", "compact", true],
    ] as const)("saves the flipped %s switch", async (name, field, expected) => {
      renderPopover();
      await userEvent.click(screen.getByRole("switch", { name }));
      expect(saved()?.timeline[field]).toBe(expected);
    });

    it("saves the chosen Titles option", async () => {
      renderPopover();
      await userEvent.click(screen.getByRole("button", { name: "App" }));
      expect(saved()?.timeline.titles).toBe("app");
    });

    it("presses the current Titles option", () => {
      renderPopover();
      expect(screen.getByRole("button", { name: "Document" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "App" })).toHaveAttribute("aria-pressed", "false");
    });
  });

  describe("app lanes", () => {
    it("lists the apps of the day with their lane switch", () => {
      renderPopover({ apps: [dayApp(), dayApp({ appKey: "com.apple.Notes", app: "Notes", ownLane: true })] });
      expect(screen.getByRole("switch", { name: "Safari lane" })).not.toBeChecked();
      expect(screen.getByRole("switch", { name: "Notes lane" })).toBeChecked();
    });

    it("gives an app a lane of its own", async () => {
      renderPopover({ apps: [dayApp()] });
      await userEvent.click(screen.getByRole("switch", { name: "Safari lane" }));
      expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Safari", { ownLane: true });
    });

    it("says so when the day has no apps", () => {
      renderPopover();
      expect(screen.getByText("No apps on this day")).toBeInTheDocument();
    });
  });

  describe("hidden apps", () => {
    it("has no count and says none is hidden at zero", () => {
      renderPopover();
      expect(screen.getByRole("heading", { name: "Hidden apps" })).toBeInTheDocument();
      expect(screen.getByText("No hidden apps")).toBeInTheDocument();
    });

    it("counts the hidden apps in the heading and lists them", () => {
      renderPopover({ hiddenApps: [mail, notes] });
      expect(screen.getByRole("heading", { name: "Hidden apps (2)" })).toBeInTheDocument();
      expect(screen.getByText("Mail")).toBeInTheDocument();
      expect(screen.getByText("Notes")).toBeInTheDocument();
      expect(screen.queryByText("No hidden apps")).not.toBeInTheDocument();
    });

    it("shows a hidden app again with Unhide", async () => {
      renderPopover({ hiddenApps: [mail, notes] });
      await userEvent.click(screen.getAllByRole("button", { name: "Unhide" })[1]);
      expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Notes", { hidden: false });
    });
  });
});
