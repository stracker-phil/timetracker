import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../../api";
import { installCommands } from "../../commands";
import { formatDayLabel, today } from "../../lib/day";
import { resetStore } from "../../state/store";
import { DAY, dayView } from "../../test/dayFixtures";
import { settings } from "../../test/settings";
import { Dialogs } from "../Dialogs";
import { DayHeader } from "./DayHeader";

const mocked = vi.mocked(api, true);

function renderHeader(patch: Parameters<typeof resetStore>[0] = { dayView: dayView(), day: DAY }) {
  resetStore({ settings: settings(), ...patch });
  installCommands();
  return render(
    <>
      <DayHeader day={DAY} totals={null} />
      <Dialogs />
    </>,
  );
}

const openMenu = () => userEvent.click(screen.getByRole("button", { name: "More day actions" }));

describe("DayHeader", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.activityCount.mockResolvedValue({ activities: 4, away: 0 });
  });

  describe("date and totals", () => {
    const totals = { recordedMs: 3_600_000, trackedMs: 7_200_000, awayMs: 600_000 };

    function renderFor(day: string) {
      resetStore({ settings: settings(), day });
      installCommands();
      render(<DayHeader day={day} totals={totals} />);
    }

    it("shows the date and the totals of a past day", () => {
      renderFor(DAY);
      expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(formatDayLabel(DAY));
      expect(screen.getByLabelText("Totals")).toHaveTextContent("recorded 1:00 · tracked 2:00 · away 0:10");
    });

    it("hides the totals on today, which the status bar shows", () => {
      renderFor(today());
      expect(screen.queryByLabelText("Totals")).not.toBeInTheDocument();
    });

    it("has no buttons to change the day", () => {
      renderFor(DAY);
      for (const name of ["Previous day", "Next day", "Today"]) expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    });
  });

  describe("lanes", () => {
    it("opens the Lanes popover from the Lanes button", async () => {
      renderHeader();
      await userEvent.click(screen.getByRole("button", { name: "Lanes" }));
      expect(await screen.findByRole("dialog", { name: "Lanes" })).toBeInTheDocument();
    });

    it("has no Lanes button in the list", () => {
      renderHeader({ dayView: dayView(), day: DAY, uiState: { ...EMPTY_UI_STATE, dayMode: "list" } });
      expect(screen.queryByRole("button", { name: "Lanes" })).not.toBeInTheDocument();
    });

    it("brings the Lanes button back with the timeline", async () => {
      renderHeader({ dayView: dayView(), day: DAY, uiState: { ...EMPTY_UI_STATE, dayMode: "list" } });
      await userEvent.click(screen.getByRole("button", { name: "Timeline" }));
      expect(screen.getByRole("button", { name: "Lanes" })).toBeInTheDocument();
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

      expect(await screen.findByRole("heading", { name: `Delete activity for ${formatDayLabel(DAY)}` })).toBeInTheDocument();
      expect(await screen.findByText(/Deletes 4 activity spans/)).toBeInTheDocument();
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
