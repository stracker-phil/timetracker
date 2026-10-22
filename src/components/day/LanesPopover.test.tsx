import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type DayApp } from "../../api";
import { resetStore } from "../../state/store";
import { DAY, dayApp, dayView } from "../../test/dayFixtures";
import { settings } from "../../test/settings";
import { Dialogs } from "../Dialogs";
import { DayHeader } from "./DayHeader";

const mocked = vi.mocked(api, true);

async function openLanes(apps: DayApp[] = [], patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore({ settings: settings(), day: DAY, dayView: dayView({ apps }), ...patch });
  render(
    <>
      <DayHeader day={DAY} totals={null} />
      <Dialogs />
    </>,
  );
  await userEvent.click(screen.getByRole("button", { name: "Lanes" }));
  return within(await screen.findByRole("dialog", { name: "Lanes" }));
}

const timelineWith = (patch: Partial<ReturnType<typeof settings>["timeline"]>) => settings({ timeline: { ...settings().timeline, ...patch } });

describe("LanesPopover", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("summary and activity lanes", () => {
    it("shows both switches on with the summary interval in its description", async () => {
      const popover = await openLanes();
      expect(popover.getByRole("switch", { name: "Summary" })).toBeChecked();
      expect(popover.getByRole("switch", { name: "Activity" })).toBeChecked();
      expect(popover.getByText("The main app of every 5 minutes.")).toBeInTheDocument();
    });

    it("shows a switch off when its lane is hidden", async () => {
      const popover = await openLanes([], { settings: timelineWith({ summaryLane: false, showAllActivity: false }) });
      expect(popover.getByRole("switch", { name: "Summary" })).not.toBeChecked();
      expect(popover.getByRole("switch", { name: "Activity" })).not.toBeChecked();
    });

    it("saves summaryLane when the Summary switch is flipped", async () => {
      const popover = await openLanes();
      await userEvent.click(popover.getByRole("switch", { name: "Summary" }));
      expect(mocked.updateSettings).toHaveBeenCalledWith(timelineWith({ summaryLane: false }));
    });

    it("saves showAllActivity when the Activity switch is flipped", async () => {
      const popover = await openLanes([], { settings: timelineWith({ showAllActivity: false }) });
      await userEvent.click(popover.getByRole("switch", { name: "Activity" }));
      expect(mocked.updateSettings).toHaveBeenCalledWith(timelineWith({ showAllActivity: true }));
    });
  });

  describe("app lanes", () => {
    const safari = dayApp({ appKey: "com.apple.Safari", app: "Safari", activeMs: 95 * 60_000, ownLane: false });
    const notes = dayApp({ appKey: "com.apple.Notes", app: "Notes", bundleId: "com.apple.Notes", activeMs: 20 * 60_000, ownLane: true });

    it("lists the day's apps in the order given, each with its tracked time", async () => {
      const popover = await openLanes([safari, notes]);
      const group = within(popover.getByRole("heading", { name: "App lanes" }).parentElement as HTMLElement);
      expect(group.getAllByRole("switch").map((s) => s.getAttribute("aria-label"))).toEqual(["Safari lane", "Notes lane"]);
      expect(group.getByText("1h 35m")).toBeInTheDocument();
      expect(group.getByText("20m")).toBeInTheDocument();
    });

    it("shows which apps have a lane of their own", async () => {
      const popover = await openLanes([safari, notes]);
      expect(popover.getByRole("switch", { name: "Safari lane" })).not.toBeChecked();
      expect(popover.getByRole("switch", { name: "Notes lane" })).toBeChecked();
    });

    it("gives an app a lane of its own when its switch is turned on", async () => {
      const popover = await openLanes([safari, notes]);
      await userEvent.click(popover.getByRole("switch", { name: "Safari lane" }));
      expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Safari", { ownLane: true });
    });

    it("takes the lane away again when its switch is turned off", async () => {
      const popover = await openLanes([safari, notes]);
      await userEvent.click(popover.getByRole("switch", { name: "Notes lane" }));
      expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Notes", { ownLane: false });
    });

    it("says there are no apps on a day without any", async () => {
      const popover = await openLanes([]);
      expect(popover.getByText("No apps on this day")).toBeInTheDocument();
      expect(popover.queryByRole("switch", { name: /lane$/ })).not.toBeInTheDocument();
    });
  });
});
