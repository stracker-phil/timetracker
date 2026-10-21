import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore, useStore } from "../state/store";
import { trackerStatus } from "../test/data";
import { TrackerStatus } from "./TrackerStatus";

const mocked = vi.mocked(api, true);
const paused = trackerStatus({ state: { kind: "paused", until: null }, text: "Paused until resumed" });

describe("TrackerStatus", () => {
  beforeEach(() => vi.clearAllMocks());

  it("says tracking is off while there is no tracker status", () => {
    resetStore({ tracker: null });
    render(<TrackerStatus />);
    expect(screen.getByText("Tracking off")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Pause" })).not.toBeInTheDocument();
  });

  it("shows the status text", () => {
    resetStore({ tracker: trackerStatus({ text: "Tracking · Safari" }) });
    render(<TrackerStatus />);
    expect(screen.getByText("Tracking · Safari")).toBeInTheDocument();
  });

  it.each([
    ["running", trackerStatus(), "Pause"],
    ["away", trackerStatus({ state: { kind: "away", since: 0, reason: "idle" } }), "Pause"],
    ["paused", paused, "Resume"],
    ["needsPermission", trackerStatus({ state: { kind: "needsPermission" } }), "Allow…"],
  ])("offers only the matching action while %s", (_, tracker, name) => {
    resetStore({ tracker });
    render(<TrackerStatus />);
    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual([name]);
  });

  it("has no action while tracking is off", () => {
    resetStore({ tracker: null });
    render(<TrackerStatus />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("Allow… shows the permission dialog again", async () => {
    resetStore({ tracker: trackerStatus({ state: { kind: "needsPermission" } }), permissionDialogHidden: true });
    render(<TrackerStatus />);
    await userEvent.click(screen.getByRole("button", { name: "Allow…" }));
    expect(useStore.getState().permissionDialogHidden).toBe(false);
  });

  describe("pausing", () => {
    beforeEach(() => resetStore({ tracker: trackerStatus() }));

    it("offers the pause durations in a menu", async () => {
      render(<TrackerStatus />);
      await userEvent.click(screen.getByRole("button", { name: "Pause" }));
      expect(screen.getAllByRole("menuitem").map((i) => i.textContent)).toEqual(["For 15 minutes", "For 1 hour", "Until tomorrow", "Until resumed"]);
    });

    it.each([
      ["For 15 minutes", { minutes: 15 }],
      ["For 1 hour", { minutes: 60 }],
      ["Until tomorrow", "tomorrow"],
      ["Until resumed", "resumed"],
    ])("sends %s to the backend", async (label, until) => {
      render(<TrackerStatus />);
      await userEvent.click(screen.getByRole("button", { name: "Pause" }));
      await userEvent.click(screen.getByRole("menuitem", { name: label }));
      expect(mocked.trackerPause).toHaveBeenCalledWith(until);
    });

    it("shows the text of the status the backend returns", async () => {
      mocked.trackerPause.mockResolvedValueOnce(paused);
      render(<TrackerStatus />);
      await userEvent.click(screen.getByRole("button", { name: "Pause" }));
      await userEvent.click(screen.getByRole("menuitem", { name: "Until resumed" }));
      expect(await screen.findByText("Paused until resumed")).toBeInTheDocument();
      expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    });
  });

  describe("while paused", () => {
    beforeEach(() => resetStore({ tracker: paused }));

    it("offers Resume instead of Pause", () => {
      render(<TrackerStatus />);
      expect(screen.getByRole("button", { name: "Resume" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Pause" })).not.toBeInTheDocument();
    });

    it("resumes tracking and shows the returned status", async () => {
      mocked.trackerResume.mockResolvedValueOnce(trackerStatus({ text: "Tracking · Mail" }));
      render(<TrackerStatus />);
      await userEvent.click(screen.getByRole("button", { name: "Resume" }));
      expect(mocked.trackerResume).toHaveBeenCalled();
      expect(await screen.findByText("Tracking · Mail")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
    });
  });
});
