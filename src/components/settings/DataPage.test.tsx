import { render, screen, waitFor, within } from "@testing-library/react";
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

async function renderDataPage(patch: Parameters<typeof dataInfo>[0] = {}, settingsPatch: Parameters<typeof settings>[0] = {}) {
  mocked.dataInfo.mockResolvedValue(dataInfo(patch));
  resetStore({ settings: settings(settingsPatch), dialog: { type: "settings", page: "data" } });
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

  describe("MCP server", () => {
    const exposeSwitch = () => screen.getByRole("switch", { name: "Expose activity to the MCP server" });

    it("keeps activity hidden from MCP clients by default", async () => {
      await renderDataPage();
      expect(exposeSwitch()).not.toBeChecked();
    });

    it("saves exposeActivity when the switch is turned on", async () => {
      await renderDataPage();
      await userEvent.click(exposeSwitch());
      expect(mocked.updateSettings).toHaveBeenCalledWith(settings({ mcp: { exposeActivity: true } }));
    });

    it("saves exposeActivity false when the switch is turned off", async () => {
      await renderDataPage({}, { mcp: { exposeActivity: true } });
      expect(exposeSwitch()).toBeChecked();
      await userEvent.click(exposeSwitch());
      expect(mocked.updateSettings).toHaveBeenCalledWith(settings({ mcp: { exposeActivity: false } }));
    });

    it.each([
      ["/Users/test/.local/bin/timetracker", "claude mcp add timetracker -- /Users/test/.local/bin/timetracker mcp"],
      [
        "/Applications/Time Tracker.app/Contents/MacOS/timetracker",
        'claude mcp add timetracker -- "/Applications/Time Tracker.app/Contents/MacOS/timetracker" mcp',
      ],
    ])("shows the add command for %s", async (binaryPath, command) => {
      await renderDataPage({ binaryPath });
      expect(screen.getByText(command)).toBeInTheDocument();
    });

    it("copies the add command and confirms with a toast", async () => {
      await renderDataPage({ binaryPath: "/Applications/Time Tracker.app/Contents/MacOS/timetracker" });
      await userEvent.click(screen.getByRole("button", { name: /Copy/ }));
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        'claude mcp add timetracker -- "/Applications/Time Tracker.app/Contents/MacOS/timetracker" mcp',
      );
      expect(await screen.findByText("Copied the command")).toBeInTheDocument();
    });
  });

  describe("Move…", () => {
    const moveButton = () => screen.getByRole("button", { name: /Move…/ });
    const sheet = () => screen.findByRole("dialog", { name: "Move the data folder?" });

    beforeEach(() => renderDataPage({ dir: "/data/dir" }));

    it("does nothing when the folder panel is cancelled", async () => {
      mocked.chooseFolder.mockResolvedValue(null);
      await userEvent.click(moveButton());
      await vi.waitFor(() => expect(mocked.chooseFolder).toHaveBeenCalled());
      expect(screen.queryByRole("dialog", { name: "Move the data folder?" })).not.toBeInTheDocument();
      expect(mocked.dataMove).not.toHaveBeenCalled();
    });

    it("asks for confirmation with both paths and the note that old files stay", async () => {
      mocked.chooseFolder.mockResolvedValue("/new/place");
      await userEvent.click(moveButton());
      const dialog = within(await sheet());
      expect(dialog.getByText("/new/place")).toBeInTheDocument();
      expect(dialog.getByText("/data/dir")).toBeInTheDocument();
      expect(dialog.getByText(/The old files stay in/)).toBeInTheDocument();
      expect(mocked.dataMove).not.toHaveBeenCalled();
    });

    it("closes the sheet without moving when Cancel is pressed", async () => {
      mocked.chooseFolder.mockResolvedValue("/new/place");
      await userEvent.click(moveButton());
      await userEvent.click(within(await sheet()).getByRole("button", { name: "Cancel" }));
      expect(screen.queryByRole("dialog", { name: "Move the data folder?" })).not.toBeInTheDocument();
      expect(mocked.dataMove).not.toHaveBeenCalled();
    });

    it("moves to the chosen folder, toasts, and shows the new location", async () => {
      mocked.chooseFolder.mockResolvedValue("/new/place");
      mocked.dataMove.mockResolvedValue(dataInfo({ dir: "/new/place" }));
      await userEvent.click(moveButton());
      mocked.dataInfo.mockResolvedValue(dataInfo({ dir: "/new/place" }));
      await userEvent.click(within(await sheet()).getByRole("button", { name: "Move" }));
      expect(mocked.dataMove).toHaveBeenCalledWith("/new/place");
      expect(await screen.findByText("Data moved to /new/place. The old files are still in /data/dir.")).toBeInTheDocument();
      expect(await screen.findByText("/new/place")).toBeInTheDocument();
      expect(mocked.getSettings).toHaveBeenCalled();
      expect(screen.queryByRole("dialog", { name: "Move the data folder?" })).not.toBeInTheDocument();
    });

    it("shows Moving… and disables the buttons while the copy runs", async () => {
      mocked.chooseFolder.mockResolvedValue("/new/place");
      let finish: (info: ReturnType<typeof dataInfo>) => void = () => {};
      mocked.dataMove.mockReturnValue(new Promise((resolve) => (finish = resolve)));
      await userEvent.click(moveButton());
      await userEvent.click(within(await sheet()).getByRole("button", { name: "Move" }));
      const dialog = within(await sheet());
      expect(dialog.getByRole("button", { name: "Moving…" })).toBeDisabled();
      expect(dialog.getByRole("button", { name: "Cancel" })).toBeDisabled();
      finish(dataInfo({ dir: "/new/place" }));
      await waitFor(() => expect(screen.queryByRole("dialog", { name: "Move the data folder?" })).not.toBeInTheDocument());
    });

    it("renders a refused target inline and closes the sheet", async () => {
      mocked.chooseFolder.mockResolvedValue("/not/empty");
      mocked.dataMove.mockRejectedValue({ kind: "invalid_input", message: "That folder already contains a database", code: null });
      await userEvent.click(moveButton());
      await userEvent.click(within(await sheet()).getByRole("button", { name: "Move" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("That folder already contains a database");
      expect(screen.queryByRole("dialog", { name: "Move the data folder?" })).not.toBeInTheDocument();
      expect(screen.queryByText(/Moving the data folder failed/)).not.toBeInTheDocument();
    });

    it("toasts other errors instead of showing them inline", async () => {
      mocked.chooseFolder.mockResolvedValue("/new/place");
      mocked.dataMove.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      await userEvent.click(moveButton());
      await userEvent.click(within(await sheet()).getByRole("button", { name: "Move" }));
      expect(await screen.findByText(/Moving the data folder failed/)).toBeInTheDocument();
      expect(within(screen.getByRole("dialog")).queryByRole("alert")).not.toBeInTheDocument();
    });

    it("clears an earlier refusal after a successful move", async () => {
      mocked.chooseFolder.mockResolvedValue("/x");
      mocked.dataMove.mockRejectedValueOnce({ kind: "invalid_input", message: "Refused", code: null });
      await userEvent.click(moveButton());
      await userEvent.click(within(await sheet()).getByRole("button", { name: "Move" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("Refused");
      mocked.dataMove.mockResolvedValue(dataInfo({ dir: "/x" }));
      await userEvent.click(moveButton());
      await userEvent.click(within(await sheet()).getByRole("button", { name: "Move" }));
      await waitFor(() => expect(within(screen.getByRole("dialog")).queryByRole("alert")).not.toBeInTheDocument());
    });
  });

  describe("Move… when the store could not be opened", () => {
    it("is disabled", async () => {
      await renderDataPage({ available: false, error: "locked" });
      expect(screen.getByRole("button", { name: /Move…/ })).toBeDisabled();
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
