import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore } from "../../state/store";
import { appStatus } from "../../test/appStatus";
import { trackerStatus } from "../../test/data";
import { renderWithClient } from "../../test/queries";
import { StatusPage } from "./StatusPage";

const mocked = vi.mocked(api, true);

describe("Settings Status page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    mocked.appStatus.mockResolvedValue(appStatus());
  });

  describe("build", () => {
    it("names a release build with its version", async () => {
      renderWithClient(<StatusPage />);
      expect(await screen.findByText("Release 1.4.0")).toBeInTheDocument();
    });

    it("names a debug build with its version", async () => {
      mocked.appStatus.mockResolvedValue(appStatus({ debug: true, capture: "demo" }));
      renderWithClient(<StatusPage />);
      expect(await screen.findByText("Debug 1.4.0")).toBeInTheDocument();
    });
  });

  describe("capture", () => {
    it.each([
      ["real", "The windows in front"],
      ["demo", "Made-up windows (debug build)"],
    ] as const)("describes the %s capture", async (capture, text) => {
      mocked.appStatus.mockResolvedValue(appStatus({ capture }));
      renderWithClient(<StatusPage />);
      expect(await screen.findByText(text)).toBeInTheDocument();
    });
  });

  describe("tracker", () => {
    it.each([
      ["granted", "Allowed"],
      ["denied", "Not allowed"],
      ["unknown", "Not asked yet"],
    ] as const)("shows the %s accessibility permission as %s", async (permission, text) => {
      mocked.trackerStatus.mockResolvedValue(trackerStatus({ permission }));
      renderWithClient(<StatusPage />);
      expect(await screen.findByText(text)).toBeInTheDocument();
    });

    it("shows the tracker's own text", async () => {
      mocked.trackerStatus.mockResolvedValue(trackerStatus({ text: "Paused until 14:00" }));
      renderWithClient(<StatusPage />);
      expect(await screen.findByText("Paused until 14:00")).toBeInTheDocument();
    });

    it("says the tracker is off when its status can't be read", async () => {
      mocked.trackerStatus.mockRejectedValue({ kind: "io", message: "No data store", code: null });
      renderWithClient(<StatusPage />);
      expect(await screen.findByText("Off: the data store isn't open")).toBeInTheDocument();
      expect(screen.getByText("Unknown")).toBeInTheDocument();
    });
  });

  describe("files", () => {
    it("shows the settings folder, the database and the log folder", async () => {
      renderWithClient(<StatusPage />);
      expect(await screen.findByText("/Users/demo/Library/Application Support/com.example.timetracker")).toBeInTheDocument();
      expect(screen.getByText("/Users/demo/Library/Application Support/com.example.timetracker/data/timetracker.sqlite")).toBeInTheDocument();
      expect(screen.getByText("/Users/demo/Library/Logs/com.example.timetracker")).toBeInTheDocument();
    });

    it("shows a moved database where Settings put it", async () => {
      mocked.appStatus.mockResolvedValue(appStatus({ database: "/Volumes/Demo/timetracker.sqlite" }));
      renderWithClient(<StatusPage />);
      expect(await screen.findByText("/Volumes/Demo/timetracker.sqlite")).toBeInTheDocument();
    });
  });

  describe("command line", () => {
    it.each([
      [{ link: "installed", path: "/Users/demo/.local/bin/timetracker" }, "Installed at /Users/demo/.local/bin/timetracker"],
      [{ link: "notInstalled", path: "/Users/demo/.local/bin/timetracker" }, "Not installed (/Users/demo/.local/bin/timetracker)"],
      [{ link: "notApplicable", path: "/Users/demo/build/timetracker" }, "Not linked for debug builds"],
    ] as const)("describes the link %j", async (commandLine, text) => {
      mocked.appStatus.mockResolvedValue(appStatus({ commandLine }));
      renderWithClient(<StatusPage />);
      expect(await screen.findByText(text)).toBeInTheDocument();
    });
  });

  it("shows an alert with the reason when the status can't be read", async () => {
    mocked.appStatus.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
    renderWithClient(<StatusPage />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't read the status: Disk full");
    expect(screen.queryByText("Build")).not.toBeInTheDocument();
  });
});
