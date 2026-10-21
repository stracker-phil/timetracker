import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../../api";
import { flushUiState, resetStore, useStore } from "../../state/store";
import { dataInfo, trackerStatus } from "../../test/data";
import { settings } from "../../test/settings";
import { Toasts } from "../Toasts";
import { Welcome } from "./Welcome";

const mocked = vi.mocked(api, true);
const needsPermission = trackerStatus({ state: { kind: "needsPermission" }, permission: "denied", text: "Needs permission" });
const sheet = () => screen.queryByRole("dialog");

function renderWelcome(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore({ settings: settings(), tracker: needsPermission, uiState: { ...EMPTY_UI_STATE, onboardingDone: false }, ...patch });
  return render(
    <>
      <Welcome />
      <Toasts />
    </>,
  );
}

const next = () => userEvent.click(screen.getByRole("button", { name: "Next" }));

describe("Welcome", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => flushUiState());

  it("is absent once the onboarding is done", () => {
    renderWelcome({ uiState: { ...EMPTY_UI_STATE, onboardingDone: true } });
    expect(sheet()).not.toBeInTheDocument();
  });

  it("opens on what is recorded and where it is kept", async () => {
    mocked.dataInfo.mockResolvedValueOnce(dataInfo({ dir: "/Users/me/Library/TimeTracker" }));
    renderWelcome();
    expect(screen.getByRole("dialog", { name: "Welcome to TimeTracker" })).toBeInTheDocument();
    expect(screen.getByText("1 of 3")).toBeInTheDocument();
    expect(await screen.findByText("/Users/me/Library/TimeTracker")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Back" })).not.toBeInTheDocument();
  });

  it("advances through the panels and back", async () => {
    renderWelcome();
    await next();
    expect(screen.getByRole("dialog", { name: "Allow reading window titles" })).toBeInTheDocument();
    expect(screen.getByText("2 of 3")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByRole("dialog", { name: "Welcome to TimeTracker" })).toBeInTheDocument();
  });

  describe("permission panel", () => {
    it("blocks Next while the permission is missing", async () => {
      renderWelcome();
      await next();
      expect(screen.getByText("Not granted yet.")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
    });

    it("enables Next once the permission is granted", async () => {
      mocked.trackerStatus.mockResolvedValueOnce(trackerStatus());
      renderWelcome();
      await next();
      await userEvent.click(screen.getByRole("button", { name: "Grant access" }));
      expect(await screen.findByText("Access granted. Window titles are recorded.")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Next" })).toBeEnabled();
    });

    it("enables Next when a tracker status event reports the permission granted", async () => {
      renderWelcome();
      await next();
      act(() => useStore.getState().setTracker(trackerStatus()));
      expect(screen.getByRole("button", { name: "Next" })).toBeEnabled();
    });

    it("stays blocked and says so when the permission is still missing after Grant access", async () => {
      mocked.requestPermission.mockResolvedValueOnce("denied");
      mocked.trackerStatus.mockResolvedValueOnce(needsPermission);
      renderWelcome();
      await next();
      await userEvent.click(screen.getByRole("button", { name: "Grant access" }));
      expect(await screen.findByText("Accessibility is not granted yet")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
    });

    it("moves on to the last panel with Skip for now", async () => {
      renderWelcome();
      await next();
      await userEvent.click(screen.getByRole("button", { name: "Skip for now" }));
      expect(screen.getByRole("dialog", { name: "Always at hand" })).toBeInTheDocument();
    });

    it("leaves Next enabled when coming back after Skip for now", async () => {
      renderWelcome();
      await next();
      await userEvent.click(screen.getByRole("button", { name: "Skip for now" }));
      await userEvent.click(screen.getByRole("button", { name: "Back" }));
      expect(screen.getByRole("button", { name: "Next" })).toBeEnabled();
    });

    it("says the tracker is off when the data store could not be opened", async () => {
      renderWelcome({ tracker: null });
      await next();
      expect(screen.getByText(/The tracker is off/)).toBeInTheDocument();
    });
  });

  describe("last panel", () => {
    async function openLastPanel() {
      renderWelcome({ tracker: trackerStatus() });
      await next();
      await next();
    }

    it("shows the launch at login switch and the timer shortcut", async () => {
      await openLastPanel();
      expect(screen.getByRole("switch", { name: "Launch at login" })).toBeInTheDocument();
      expect(screen.getByText("⌘⇧T")).toBeInTheDocument();
    });

    it("turns launch at login on from the sheet", async () => {
      await openLastPanel();
      await userEvent.click(screen.getByRole("switch", { name: "Launch at login" }));
      expect(mocked.autostartSet).toHaveBeenLastCalledWith(true);
    });

    it("saves that the welcome is done and closes on Done", async () => {
      await openLastPanel();
      await userEvent.click(screen.getByRole("button", { name: "Done" }));
      expect(mocked.onboardingDone).toHaveBeenCalled();
      expect(sheet()).not.toBeInTheDocument();
    });

    it("does not come back after it is done", async () => {
      await openLastPanel();
      await userEvent.click(screen.getByRole("button", { name: "Done" }));
      act(() => useStore.getState().setTracker(needsPermission));
      expect(sheet()).not.toBeInTheDocument();
    });

    it("toasts when the finished welcome cannot be saved, and stays closed", async () => {
      mocked.onboardingDone.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await openLastPanel();
      await userEvent.click(screen.getByRole("button", { name: "Done" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("Could not save that the welcome was finished: Disk full");
      expect(sheet()).not.toBeInTheDocument();
    });
  });

  it("finishes the welcome with Escape from any panel", async () => {
    renderWelcome();
    await userEvent.keyboard("{Escape}");
    expect(mocked.onboardingDone).toHaveBeenCalled();
    expect(sheet()).not.toBeInTheDocument();
  });

  it("starts on the first panel when shown again", async () => {
    renderWelcome({ tracker: trackerStatus() });
    await next();
    await next();
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    act(() => useStore.getState().showWelcome());
    expect(await screen.findByRole("dialog", { name: "Welcome to TimeTracker" })).toBeInTheDocument();
  });
});
