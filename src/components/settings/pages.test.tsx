import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../../api";
import { installCommands } from "../../commands";
import { markSettingsWindow } from "../../lib/settingsWindow";
import { resetStore, useStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

function renderGeneral(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore({ settings: settings(), ...patch });
  installCommands();
  return render(<SettingsWindow page="general" onPage={() => {}} />);
}

describe("Settings General page", () => {
  beforeEach(() => vi.clearAllMocks());

  it("offers the launch at login switch", () => {
    renderGeneral();
    expect(screen.getByRole("switch", { name: "Launch at login" })).toBeInTheDocument();
  });

  describe("Show welcome again", () => {
    let reset: (() => void) | undefined;
    afterEach(() => reset?.());

    it("shows the welcome sheet again in the main window", async () => {
      renderGeneral({ uiState: { ...EMPTY_UI_STATE, onboardingDone: true } });
      await userEvent.click(screen.getByRole("button", { name: "Show welcome again" }));
      expect(useStore.getState().uiState.onboardingDone).toBe(false);
      expect(mocked.showWelcome).not.toHaveBeenCalled();
    });

    it("asks Rust to show it in the main window when pressed in the Settings window", async () => {
      reset = markSettingsWindow();
      renderGeneral({ uiState: { ...EMPTY_UI_STATE, onboardingDone: true } });
      await userEvent.click(screen.getByRole("button", { name: "Show welcome again" }));
      expect(mocked.showWelcome).toHaveBeenCalled();
      expect(useStore.getState().uiState.onboardingDone).toBe(true);
    });

    it("toasts when Rust cannot show it", async () => {
      reset = markSettingsWindow();
      mocked.showWelcome.mockRejectedValueOnce({ kind: "io", message: "No window", code: null });
      renderGeneral();
      await userEvent.click(screen.getByRole("button", { name: "Show welcome again" }));
      await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message).join()).toMatch(/Couldn't show the welcome sheet/));
    });
  });
});
