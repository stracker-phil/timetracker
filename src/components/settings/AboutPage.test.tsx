import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { appStatus } from "../../test/appStatus";
import { renderWithClient } from "../../test/queries";
import { AboutPage } from "./AboutPage";

const mocked = vi.mocked(api, true);

describe("Settings About page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    mocked.appStatus.mockResolvedValue(appStatus({ version: "1.4.0" }));
  });

  it("shows the version", async () => {
    renderWithClient(<AboutPage />);
    expect(await screen.findByText("1.4.0")).toBeInTheDocument();
  });

  it("still lists its rows while the status has not loaded", () => {
    mocked.appStatus.mockReturnValue(new Promise(() => {}));
    renderWithClient(<AboutPage />);
    expect(screen.getByText("Version")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Open README/ })).toBeInTheDocument();
  });

  it.each([
    ["Show Data Folder", "app:dataFolder"],
    ["Open Log Folder", "app:logs"],
    ["Open README", "app:readme"],
  ])("runs %s as the app command %s", async (name, command) => {
    renderWithClient(<AboutPage />);
    await userEvent.click(screen.getByRole("button", { name: new RegExp(name) }));
    expect(mocked.runCommand).toHaveBeenCalledWith(command);
  });

  it("toasts the reason when the command fails", async () => {
    mocked.runCommand.mockRejectedValueOnce({ kind: "io", message: "Folder is gone", code: null });
    renderWithClient(<AboutPage />);
    await userEvent.click(screen.getByRole("button", { name: /Show Data Folder/ }));
    await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message)).toContain("Couldn't show the data folder: Folder is gone"));
  });
});
