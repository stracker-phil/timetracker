import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type AppOverride } from "../../api";
import { installCommands } from "../../commands";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsDialog } from "./SettingsDialog";

const mocked = vi.mocked(api, true);
const override = (patch: Partial<AppOverride> = {}): AppOverride => ({ groupByApp: null, titles: null, hidden: false, ownLane: false, ...patch });

function renderPage(apps: Record<string, AppOverride>) {
  resetStore({ settings: settings({ apps }), dialog: { type: "settings", page: "timeline" } });
  installCommands();
  return render(<SettingsDialog />);
}

describe("Settings Timeline page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.appsSeen.mockResolvedValue([{ bundleId: "com.apple.Notes", app: "Notes", lastSeen: 1, totalMs: 60_000 }]);
  });

  it("says no apps are hidden when none is", () => {
    renderPage({ "com.apple.Safari": override({ ownLane: true }) });
    expect(screen.getByText("No hidden apps")).toBeInTheDocument();
  });

  it("lists each hidden app with its name and bundle id, and not the visible ones", async () => {
    renderPage({ "com.apple.Notes": override({ hidden: true }), "com.example.gone": override({ hidden: true }), "com.apple.Safari": override() });
    expect(await screen.findByText("Notes")).toBeInTheDocument();
    expect(screen.getByText("com.apple.Notes")).toBeInTheDocument();
    expect(screen.getAllByText("com.example.gone").length).toBeGreaterThan(0);
    expect(screen.queryByText("com.apple.Safari")).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Show" })).toHaveLength(2);
  });

  it("shows an app again through the api", async () => {
    renderPage({ "com.apple.Notes": override({ hidden: true }) });
    await userEvent.click(await screen.findByRole("button", { name: "Show" }));
    expect(mocked.setAppSetting).toHaveBeenCalledWith("com.apple.Notes", { hidden: false });
  });
});
