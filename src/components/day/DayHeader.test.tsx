import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { installCommands, runCommand } from "../../commands";
import { today } from "../../lib/day";
import { resetStore } from "../../state/store";
import { DAY, dayView } from "../../test/dayFixtures";
import { settings } from "../../test/settings";
import { Dialogs } from "../Dialogs";
import { DayHeader } from "./DayHeader";

const mocked = vi.mocked(api, true);

function renderHeader(patch: Parameters<typeof resetStore>[0] = { dayView: dayView({ label: "Tuesday, March 10, 2026" }), day: DAY }) {
  resetStore({ settings: settings(), ...patch });
  installCommands();
  return render(
    <>
      <DayHeader totals={null} />
      <Dialogs />
    </>,
  );
}

const openMenu = () => userEvent.click(screen.getByRole("button", { name: "More day actions" }));

describe("DayHeader", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.activityCount.mockResolvedValue({ activities: 4, away: 0, rangeLabel: "Tue, Mar 10 00:00 – Wed, Mar 11 00:00" });
  });

  describe("date and totals", () => {
    const totals = { recordedMs: 3_600_000, trackedMs: 7_200_000, awayMs: 600_000 };

    function renderFor(label: string, shown = totals, day = DAY) {
      resetStore({ settings: settings(), day });
      installCommands();
      render(<DayHeader label={label} totals={shown} />);
    }

    it.each(["Tuesday, March 10", "Tue, Mar 10"])("shows the title %s Rust gives for the day, as is", (label) => {
      renderFor(label);
      expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(new RegExp(`^${label}$`));
    });

    it("shows the totals of a past day", () => {
      renderFor("Tuesday, March 10");
      expect(screen.getByLabelText("Totals")).toHaveTextContent("recorded 1h 00m · tracked 2h 00m · away 10m");
    });

    it("shows the totals on today too", () => {
      renderFor("Today", totals, today());
      expect(screen.getByLabelText("Totals")).toHaveTextContent("recorded 1h 00m · tracked 2h 00m · away 10m");
    });

    it.each([
      ["recorded", { recordedMs: 0, trackedMs: 7_200_000, awayMs: 600_000 }, "tracked 2h 00m · away 10m"],
      ["tracked", { recordedMs: 3_600_000, trackedMs: 0, awayMs: 600_000 }, "recorded 1h 00m · away 10m"],
      ["away", { recordedMs: 3_600_000, trackedMs: 7_200_000, awayMs: 0 }, "recorded 1h 00m · tracked 2h 00m"],
    ])("leaves out the %s part when it is zero", (_part, shown, expected) => {
      renderFor("Tuesday, March 10", shown);
      expect(screen.getByLabelText("Totals")).toHaveTextContent(new RegExp(`^${expected}$`));
    });

    it("shows no totals when everything is zero", () => {
      renderFor("Tuesday, March 10", { recordedMs: 0, trackedMs: 0, awayMs: 0 });
      expect(screen.queryByLabelText("Totals")).not.toBeInTheDocument();
    });

    it("has no buttons to change the day", () => {
      renderFor("Tuesday, March 10");
      for (const name of ["Previous day", "Next day", "Today"]) expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    });
  });

  describe("view options", () => {
    it("opens the View options popover from the View options button", async () => {
      renderHeader();
      await userEvent.click(screen.getByRole("button", { name: "View options" }));
      expect(await screen.findByRole("dialog", { name: "View options" })).toBeInTheDocument();
    });

    it("opens the popover with the day.settings command", async () => {
      renderHeader();
      act(() => {
        runCommand("day.settings");
      });
      expect(await screen.findByRole("dialog", { name: "View options" })).toBeInTheDocument();
    });

    it("has no Lanes button and no Timeline/List switch", () => {
      renderHeader();
      for (const name of ["Lanes", "Timeline", "List"]) expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    });
  });

  describe("more actions menu", () => {
    it("lists the delete entries", async () => {
      renderHeader();
      await openMenu();
      expect(screen.getByRole("menuitem", { name: "Delete activity for this day…" })).toBeInTheDocument();
      expect(screen.getByRole("menuitem", { name: "Delete activity in a range…" })).toBeInTheDocument();
    });

    it("opens the delete dialog for the shown day", async () => {
      renderHeader();
      await openMenu();
      await userEvent.click(screen.getByRole("menuitem", { name: "Delete activity for this day…" }));

      expect(await screen.findByRole("heading", { name: "Delete activity for Tuesday, March 10, 2026" })).toBeInTheDocument();
      expect(await screen.findByText(/Deletes 4 activities/)).toBeInTheDocument();
    });

    it("opens the range dialog from the range entry", async () => {
      renderHeader();
      await openMenu();
      await userEvent.click(screen.getByRole("menuitem", { name: "Delete activity in a range…" }));

      expect(await screen.findByRole("heading", { name: "Delete activity in a range" })).toBeInTheDocument();
      expect(screen.getByLabelText("From")).toBeInTheDocument();
    });

    it("opens no dialog while the shown day has not loaded", async () => {
      renderHeader({ dayView: null, day: DAY });
      await openMenu();
      await userEvent.click(screen.getByRole("menuitem", { name: "Delete activity for this day…" }));

      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });
});
