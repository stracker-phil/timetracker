import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { installCommands } from "../commands";
import { defaultFilter } from "../state/slices/reports";
import { resetStore, useStore } from "../state/store";
import { savedReport } from "../test/reportFixtures";
import { settings } from "../test/settings";
import { Dialogs } from "./Dialogs";
import { Sidebar } from "./Sidebar";

const mocked = vi.mocked(api, true);

const weekly = savedReport({ id: "s1", name: "Weekly", filter: { ...defaultFilter(null), range: { kind: "lastWeek" }, groupBy: "tag" } });
const monthly = savedReport({ id: "s2", name: "Monthly", filter: { ...defaultFilter(null), range: { kind: "lastMonth" } } });

function renderSidebar(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore({ settings: settings(), savedReports: [weekly, monthly], ...patch });
  installCommands();
  render(
    <>
      <Sidebar />
      <Dialogs />
    </>,
  );
}

const saved = () => screen.getByRole("group", { name: "Saved reports" });

describe("Sidebar saved reports", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists the saved reports under Reports", () => {
    renderSidebar();
    expect(within(saved()).getAllByRole("button").map((b) => b.textContent)).toEqual(["Weekly", "Monthly"]);
  });

  it("shows no group when nothing is saved", () => {
    renderSidebar({ savedReports: [] });
    expect(screen.queryByRole("group", { name: "Saved reports" })).not.toBeInTheDocument();
  });

  it("opens the Reports view with the filter of the clicked report", async () => {
    renderSidebar({ view: "day" });
    await userEvent.click(within(saved()).getByRole("button", { name: "Weekly" }));
    expect(useStore.getState().view).toBe("reports");
    expect(useStore.getState().reportFilter).toEqual(weekly.filter);
  });

  describe("context menu", () => {
    it("renames a report through a prefilled prompt", async () => {
      mocked.savedReportRename.mockResolvedValue(savedReport({ id: "s1", name: "Last week" }));
      renderSidebar();
      await userEvent.pointer({ keys: "[MouseRight]", target: within(saved()).getByRole("button", { name: "Weekly" }) });
      await userEvent.click(await screen.findByRole("menuitem", { name: "Rename…" }));
      const input = await screen.findByRole("textbox", { name: "Name" });
      expect(input).toHaveValue("Weekly");
      await userEvent.clear(input);
      await userEvent.type(input, "Last week{Enter}");
      expect(mocked.savedReportRename).toHaveBeenCalledWith("s1", "Last week");
      expect(await within(saved()).findByRole("button", { name: "Last week" })).toBeInTheDocument();
      expect(within(saved()).queryByRole("button", { name: "Weekly" })).not.toBeInTheDocument();
    });

    it("deletes a report after confirming", async () => {
      renderSidebar();
      await userEvent.pointer({ keys: "[MouseRight]", target: within(saved()).getByRole("button", { name: "Monthly" }) });
      await userEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
      expect(await screen.findByText(/Delete the saved report "Monthly"/)).toBeInTheDocument();
      expect(mocked.savedReportDelete).not.toHaveBeenCalled();
      await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" }));
      expect(mocked.savedReportDelete).toHaveBeenCalledWith("s2");
      await vi.waitFor(() => expect(within(saved()).queryByRole("button", { name: "Monthly" })).not.toBeInTheDocument());
      expect(within(saved()).getByRole("button", { name: "Weekly" })).toBeInTheDocument();
    });

    it("keeps the report when the delete is cancelled", async () => {
      renderSidebar();
      await userEvent.pointer({ keys: "[MouseRight]", target: within(saved()).getByRole("button", { name: "Monthly" }) });
      await userEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
      await userEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Cancel" }));
      expect(mocked.savedReportDelete).not.toHaveBeenCalled();
      expect(within(saved()).getByRole("button", { name: "Monthly" })).toBeInTheDocument();
    });
  });
});
