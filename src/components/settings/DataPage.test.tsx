import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { installCommands } from "../../commands";
import { resetStore } from "../../state/store";
import { dataInfo } from "../../test/data";
import { settings } from "../../test/settings";
import { Toasts } from "../Toasts";
import { SettingsDialog } from "./SettingsDialog";

const mocked = vi.mocked(api, true);

async function renderDataPage(patch: Parameters<typeof dataInfo>[0] = {}) {
  mocked.dataInfo.mockResolvedValue(dataInfo(patch));
  resetStore({ settings: settings(), dialog: { type: "settings", page: "data" } });
  installCommands();
  render(
    <>
      <SettingsDialog />
      <Toasts />
    </>,
  );
  await screen.findByRole("button", { name: /Export backup/ });
}

describe("Settings Data page", () => {
  beforeEach(() => vi.clearAllMocks());

  it("is reachable from the navigation", async () => {
    mocked.dataInfo.mockResolvedValue(dataInfo());
    resetStore({ settings: settings(), dialog: { type: "settings" } });
    installCommands();
    render(<SettingsDialog />);
    await userEvent.click(within(screen.getByRole("navigation", { name: "Settings pages" })).getByRole("button", { name: "Data" }));
    expect(await screen.findByRole("button", { name: /Export backup/ })).toBeInTheDocument();
  });

  describe("when the store is available", () => {
    beforeEach(() =>
      renderDataPage({
        dir: "/data/dir",
        file: "/data/dir/timetracker.sqlite",
        activities: 12345,
        recordings: 7,
        projects: 3,
        firstActivity: new Date(2026, 0, 5, 12).getTime(),
        sizeBytes: 1536,
      }),
    );

    it("shows the folder, the grouped counts, the first activity day and the size", () => {
      expect(screen.getByText("/data/dir")).toBeInTheDocument();
      expect(screen.getByText("12,345")).toBeInTheDocument();
      expect(screen.getByText("7")).toBeInTheDocument();
      expect(screen.getByText("3")).toBeInTheDocument();
      expect(screen.getByText("2026-01-05")).toBeInTheDocument();
      expect(screen.getByText("1.5 KB")).toBeInTheDocument();
    });

    it("reveals the database file in Finder", async () => {
      await userEvent.click(screen.getByRole("button", { name: /Reveal in Finder/ }));
      expect(mocked.revealPath).toHaveBeenCalledWith("/data/dir/timetracker.sqlite");
    });
  });

  it("says None yet when nothing was recorded", async () => {
    await renderDataPage({ firstActivity: null });
    expect(screen.getByText("None yet")).toBeInTheDocument();
  });

  describe("when the store could not be opened", () => {
    beforeEach(() => renderDataPage({ available: false, error: "database is locked", activities: 99 }));

    it("shows the error in an alert instead of the counts", () => {
      expect(screen.getByRole("alert")).toHaveTextContent("database is locked");
      expect(screen.queryByText("Activity spans")).not.toBeInTheDocument();
      expect(screen.queryByText("99")).not.toBeInTheDocument();
    });

    it("disables the backup export", () => {
      expect(screen.getByRole("button", { name: /Export backup/ })).toBeDisabled();
    });
  });

  describe("Export backup", () => {
    beforeEach(() => renderDataPage());

    it("writes the backup to the chosen path and confirms with a toast", async () => {
      mocked.chooseSavePath.mockResolvedValue("/tmp/backup.json");
      mocked.backupExport.mockResolvedValue({ bytes: 10, recordings: 1234, activities: 56 });
      await userEvent.click(screen.getByRole("button", { name: /Export backup/ }));
      expect(await screen.findByText("Backup written: 1,234 recordings, 56 activity spans")).toBeInTheDocument();
      expect(mocked.chooseSavePath).toHaveBeenCalledWith(expect.stringMatching(/^timetracker-backup-\d{4}-\d{2}-\d{2}$/), "json");
      expect(mocked.backupExport).toHaveBeenCalledWith("/tmp/backup.json");
    });

    it("does nothing when the save panel is cancelled", async () => {
      mocked.chooseSavePath.mockResolvedValue(null);
      await userEvent.click(screen.getByRole("button", { name: /Export backup/ }));
      await vi.waitFor(() => expect(mocked.chooseSavePath).toHaveBeenCalled());
      expect(mocked.backupExport).not.toHaveBeenCalled();
      expect(screen.queryByText(/Backup written/)).not.toBeInTheDocument();
    });

    it("shows an error toast when the export fails", async () => {
      mocked.chooseSavePath.mockResolvedValue("/tmp/backup.json");
      mocked.backupExport.mockRejectedValue({ kind: "io", message: "disk full", code: null });
      await userEvent.click(screen.getByRole("button", { name: /Export backup/ }));
      expect(await screen.findByText(/Backup failed: disk full/)).toBeInTheDocument();
      expect(screen.queryByText(/Backup written/)).not.toBeInTheDocument();
    });
  });
});
