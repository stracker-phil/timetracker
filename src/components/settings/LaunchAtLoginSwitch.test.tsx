import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Settings } from "../../api";
import { resetStore } from "../../state/store";
import { queryData, renderWithClient, seedQuery, SETTINGS_KEY } from "../../test/queries";
import { settings } from "../../test/settings";
import { Toasts } from "../Toasts";
import { LaunchAtLoginSwitch } from "./LaunchAtLoginSwitch";

const mocked = vi.mocked(api, true);
const toggle = () => screen.getByRole("switch", { name: "Launch at login" });
const storedSetting = () => queryData<Settings>(SETTINGS_KEY)?.launchAtLogin;

function renderSwitch() {
  seedQuery(SETTINGS_KEY, settings({ launchAtLogin: false }));
  renderWithClient(
    <>
      <LaunchAtLoginSwitch />
      <Toasts />
    </>,
  );
}

describe("LaunchAtLoginSwitch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it("is off when the LaunchAgent is not installed", async () => {
    renderSwitch();
    await vi.waitFor(() => expect(mocked.autostartStatus).toHaveBeenCalled());
    expect(toggle()).not.toBeChecked();
  });

  it("is on when the LaunchAgent is installed, whatever the stored setting says", async () => {
    mocked.autostartStatus.mockResolvedValueOnce(true);
    renderSwitch();
    await vi.waitFor(() => expect(toggle()).toBeChecked());
  });

  it("asks to install the LaunchAgent and shows it on", async () => {
    renderSwitch();
    await userEvent.click(toggle());
    expect(mocked.autostartSet).toHaveBeenLastCalledWith(true);
    await vi.waitFor(() => expect(toggle()).toBeChecked());
    expect(storedSetting()).toBe(true);
  });

  it("asks to remove the LaunchAgent and shows it off", async () => {
    mocked.autostartStatus.mockResolvedValueOnce(true);
    renderSwitch();
    await vi.waitFor(() => expect(toggle()).toBeChecked());
    await userEvent.click(toggle());
    expect(mocked.autostartSet).toHaveBeenLastCalledWith(false);
    await vi.waitFor(() => expect(toggle()).not.toBeChecked());
  });

  it("toasts and keeps the previous state when changing fails", async () => {
    mocked.autostartSet.mockRejectedValueOnce({ kind: "io", message: "Permission denied", code: null });
    renderSwitch();
    await userEvent.click(toggle());
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not change launch at login: Permission denied");
    expect(toggle()).not.toBeChecked();
    expect(storedSetting()).toBe(false);
  });

  it("shows the state the backend reports when it differs from the request", async () => {
    mocked.autostartSet.mockResolvedValueOnce(false);
    renderSwitch();
    await userEvent.click(toggle());
    await vi.waitFor(() => expect(mocked.autostartSet).toHaveBeenCalled());
    expect(toggle()).not.toBeChecked();
  });
});
