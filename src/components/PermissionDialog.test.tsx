import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../api";
import { resetStore, useStore } from "../state/store";
import { trackerStatus } from "../test/data";
import { PermissionDialog } from "./PermissionDialog";
import { Toasts } from "./Toasts";

const mocked = vi.mocked(api, true);
const needsPermission = trackerStatus({ state: { kind: "needsPermission" }, permission: "denied", text: "Needs permission" });

function renderDialog() {
  render(
    <>
      <PermissionDialog />
      <Toasts />
    </>,
  );
}

describe("PermissionDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it.each([
    ["running", trackerStatus()],
    ["paused", trackerStatus({ state: { kind: "paused", until: null } })],
    ["unloaded", null],
  ])("is absent while the tracker is %s", (_, tracker) => {
    resetStore({ tracker });
    renderDialog();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("waits while the welcome sheet is shown, then appears", () => {
    resetStore({ tracker: needsPermission, uiState: { ...EMPTY_UI_STATE, onboardingDone: false } });
    renderDialog();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    act(() => useStore.setState({ uiState: { ...EMPTY_UI_STATE, onboardingDone: true } }));
    expect(screen.getByRole("dialog", { name: "Allow TimeTracker to read window titles" })).toBeInTheDocument();
  });

  it("explains the permission when it is needed", () => {
    resetStore({ tracker: needsPermission });
    renderDialog();
    expect(screen.getByRole("dialog", { name: "Allow TimeTracker to read window titles" })).toBeInTheDocument();
  });

  it("opens the privacy settings", async () => {
    resetStore({ tracker: needsPermission });
    renderDialog();
    await userEvent.click(screen.getByRole("button", { name: "Open System Settings" }));
    expect(mocked.openPrivacySettings).toHaveBeenCalled();
  });

  it("shows a toast when the privacy settings cannot be opened", async () => {
    mocked.openPrivacySettings.mockRejectedValueOnce({ kind: "io", message: "No such pane", code: null });
    resetStore({ tracker: needsPermission });
    renderDialog();
    await userEvent.click(screen.getByRole("button", { name: "Open System Settings" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not open System Settings: No such pane");
  });

  it("closes after Check again when the tracker then reports running", async () => {
    resetStore({ tracker: needsPermission });
    renderDialog();
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(mocked.requestPermission).toHaveBeenCalled();
    await vi.waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("stays open and says so when the permission is still missing", async () => {
    mocked.requestPermission.mockResolvedValueOnce("denied");
    mocked.trackerStatus.mockResolvedValueOnce(needsPermission);
    resetStore({ tracker: needsPermission });
    renderDialog();
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(await screen.findByText("Accessibility is not granted yet")).toBeInTheDocument();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("closes with Escape", async () => {
    resetStore({ tracker: needsPermission });
    renderDialog();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
