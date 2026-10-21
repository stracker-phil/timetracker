import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { installCommands } from "../../commands";
import { formatDayLabel } from "../../lib/day";
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
