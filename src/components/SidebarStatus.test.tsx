import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { installCommands } from "../commands";
import { resetStore, useStore } from "../state/store";
import { trackerStatus } from "../test/data";
import { SidebarStatus } from "./SidebarStatus";

const mocked = vi.mocked(api, true);
const paused = trackerStatus({ state: { kind: "paused", until: null }, text: "Paused until resumed" });

describe("SidebarStatus", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    installCommands();
  });

  it("shows the status text", () => {
    resetStore({ tracker: trackerStatus({ text: "Tracking · Safari" }) });
    render(<SidebarStatus />);
    expect(screen.getByRole("status")).toHaveTextContent("Tracking · Safari");
  });

  it("says tracking is off and offers no action without a tracker status", () => {
    resetStore({ tracker: null });
    render(<SidebarStatus />);
    expect(screen.getByRole("status")).toHaveTextContent("Tracking off");
    expect(screen.queryByRole("button", { name: /tracking|Allow/ })).not.toBeInTheDocument();
  });

  describe("while tracking", () => {
    beforeEach(() => resetStore({ tracker: trackerStatus() }));

    it("pauses until resumed with the Pause button", async () => {
      mocked.trackerPause.mockResolvedValueOnce(paused);
      render(<SidebarStatus />);
      await userEvent.click(screen.getByRole("button", { name: "Pause tracking" }));
      expect(mocked.trackerPause).toHaveBeenCalledWith("resumed");
      expect(await screen.findByRole("status")).toHaveTextContent("Paused until resumed");
      expect(screen.getByRole("button", { name: "Resume tracking" })).toBeInTheDocument();
    });

    it("lists the durations on a right-click", async () => {
      render(<SidebarStatus />);
      await userEvent.pointer({ keys: "[MouseRight]", target: screen.getByRole("button", { name: "Pause tracking" }) });
      expect((await screen.findAllByRole("menuitem")).map((i) => i.textContent)).toEqual(["For 15 minutes", "For 1 hour", "Until tomorrow", "Until resumed"]);
    });

    it("pauses for the chosen duration", async () => {
      render(<SidebarStatus />);
      await userEvent.pointer({ keys: "[MouseRight]", target: screen.getByRole("button", { name: "Pause tracking" }) });
      await userEvent.click(await screen.findByRole("menuitem", { name: "For 15 minutes" }));
      expect(mocked.trackerPause).toHaveBeenCalledWith({ minutes: 15 });
    });

    it("offers Pause while away too", () => {
      resetStore({ tracker: trackerStatus({ state: { kind: "away", since: 0, reason: "idle" } }) });
      render(<SidebarStatus />);
      expect(screen.getByRole("button", { name: "Pause tracking" })).toBeInTheDocument();
    });
  });

  it("offers Resume instead of Pause while paused and resumes tracking", async () => {
    mocked.trackerResume.mockResolvedValueOnce(trackerStatus({ text: "Tracking · Mail" }));
    resetStore({ tracker: paused });
    render(<SidebarStatus />);
    expect(screen.queryByRole("button", { name: "Pause tracking" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Resume tracking" }));
    expect(await screen.findByText("Tracking · Mail")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Pause tracking" })).toBeInTheDocument();
  });

  it("offers Allow… without permission, which shows the permission dialog again", async () => {
    resetStore({ tracker: trackerStatus({ state: { kind: "needsPermission" } }), permissionDialogHidden: true });
    render(<SidebarStatus />);
    expect(screen.queryByRole("button", { name: /tracking$/ })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Allow…" }));
    expect(useStore.getState().permissionDialogHidden).toBe(false);
  });

  it("opens Settings with the gear", async () => {
    resetStore({ tracker: trackerStatus() });
    render(<SidebarStatus />);
    await userEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(useStore.getState().dialog).toEqual({ type: "settings" });
  });
});
