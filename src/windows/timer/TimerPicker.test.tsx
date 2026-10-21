import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { projectItem } from "../../test/projectFixtures";
import { recording } from "../../test/recordingFixtures";
import { recentTimer, runningStatus } from "../../test/timerFixtures";
import { TimerPicker } from "./TimerPicker";

const mocked = vi.mocked(api, true);

const website = projectItem({ project: { id: "p-site", name: "Website" } });

function renderPicker() {
  resetStore();
  mocked.projectsList.mockResolvedValue([website]);
  return render(<TimerPicker />);
}

const startButton = () => screen.getByRole("button", { name: "Start" });
const noteField = () => screen.getByRole("textbox", { name: "Note" });

async function chooseProject() {
  await userEvent.click(await screen.findByRole("option", { name: "Website" }));
}

describe("TimerPicker", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.timerStatus.mockResolvedValue({ running: null });
    mocked.timersRecent.mockResolvedValue([]);
  });

  describe("without a running timer", () => {
    it("shows the start form with Start disabled until a project is chosen", async () => {
      renderPicker();
      expect(await screen.findByRole("heading", { name: "Start timer" })).toBeInTheDocument();
      expect(startButton()).toBeDisabled();
      await chooseProject();
      expect(startButton()).toBeEnabled();
    });

    it("starts the timer with project, trimmed note and tags, then closes", async () => {
      renderPicker();
      await chooseProject();
      await userEvent.type(noteField(), "  standup ");
      await userEvent.click(screen.getByRole("combobox", { name: "Tags" }));
      await userEvent.click(screen.getByRole("option", { name: "support" }));
      await userEvent.click(startButton());
      expect(mocked.timerStart).toHaveBeenCalledWith("p-site", "standup", ["tag-support"]);
      await vi.waitFor(() => expect(mocked.timerPickerClose).toHaveBeenCalled());
    });

    it("starts with Enter in the note field", async () => {
      renderPicker();
      await chooseProject();
      await userEvent.type(noteField(), "review{Enter}");
      expect(mocked.timerStart).toHaveBeenCalledWith("p-site", "review", []);
      await vi.waitFor(() => expect(mocked.timerPickerClose).toHaveBeenCalled());
    });

    it("turns pasted line breaks in the note into spaces", async () => {
      renderPicker();
      await chooseProject();
      await userEvent.click(noteField());
      await userEvent.paste("fix\nbug");
      expect(noteField()).toHaveValue("fix bug");
    });

    it("stays open and toasts when a recording already covers now", async () => {
      mocked.timerStart.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps", code: null });
      renderPicker();
      await chooseProject();
      await userEvent.click(startButton());
      await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message)).toEqual(["A recording already covers now"]));
      expect(mocked.timerPickerClose).not.toHaveBeenCalled();
    });

    it("closes without starting on Cancel", async () => {
      renderPicker();
      await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
      expect(mocked.timerPickerClose).toHaveBeenCalled();
      expect(mocked.timerStart).not.toHaveBeenCalled();
    });

    it("closes on Escape", async () => {
      renderPicker();
      await userEvent.click(await screen.findByRole("textbox", { name: "Note" }));
      await userEvent.keyboard("{Escape}");
      expect(mocked.timerPickerClose).toHaveBeenCalled();
    });

    it("starts a recent timer with its project, note and tags when clicked", async () => {
      mocked.timersRecent.mockResolvedValue([recentTimer({ projectId: "p-site", note: "standup", tagIds: ["tag-admin"] }), recentTimer({ projectId: "p-docs", projectName: "Docs" })]);
      renderPicker();
      expect(await screen.findByRole("button", { name: "Docs" })).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Website · standup" }));
      expect(mocked.timerStart).toHaveBeenCalledWith("p-site", "standup", ["tag-admin"]);
      await vi.waitFor(() => expect(mocked.timerPickerClose).toHaveBeenCalled());
    });

    it("shows no recent list when there are no recent timers", async () => {
      renderPicker();
      await screen.findByRole("heading", { name: "Start timer" });
      expect(screen.queryByRole("list", { name: "Recent timers" })).not.toBeInTheDocument();
    });
  });

  describe("with a running timer", () => {
    beforeEach(() => {
      mocked.timerStatus.mockResolvedValue(runningStatus({ projectName: "Docs", recording: recording({ end: null, note: "writing" }) }));
      mocked.timersRecent.mockResolvedValue([recentTimer()]);
    });

    it("shows the project, the elapsed time and the note instead of the form", async () => {
      renderPicker();
      expect(await screen.findByText("Docs")).toBeInTheDocument();
      expect(screen.getByText("1:23")).toBeInTheDocument();
      expect(screen.getByText("writing")).toBeInTheDocument();
      expect(screen.queryByRole("heading", { name: "Start timer" })).not.toBeInTheDocument();
      expect(screen.queryByRole("list", { name: "Recent timers" })).not.toBeInTheDocument();
    });

    it("stops the timer and closes on Stop", async () => {
      renderPicker();
      await userEvent.click(await screen.findByRole("button", { name: "Stop" }));
      expect(mocked.timerStop).toHaveBeenCalled();
      await vi.waitFor(() => expect(mocked.timerPickerClose).toHaveBeenCalled());
    });

    it("shows the start form on Start another…", async () => {
      renderPicker();
      await userEvent.click(await screen.findByRole("button", { name: "Start another…" }));
      expect(screen.getByRole("heading", { name: "Start timer" })).toBeInTheDocument();
      expect(startButton()).toBeDisabled();
    });

    it("closes on Escape", async () => {
      renderPicker();
      await screen.findByRole("button", { name: "Stop" });
      await userEvent.keyboard("{Escape}");
      expect(mocked.timerPickerClose).toHaveBeenCalled();
    });
  });

  describe("when the window is shown again", () => {
    it("reloads the timer and follows a timer started elsewhere", async () => {
      renderPicker();
      await screen.findByRole("heading", { name: "Start timer" });
      mocked.timerStatus.mockResolvedValue(runningStatus({ projectName: "Docs" }));
      act(() => void window.dispatchEvent(new Event("focus")));
      expect(await screen.findByRole("button", { name: "Stop" })).toBeInTheDocument();
    });

    it("follows timer status events", async () => {
      renderPicker();
      await screen.findByRole("heading", { name: "Start timer" });
      act(() => mocked.onTimerStatus.mock.calls[0][0](runningStatus({ projectName: "Docs" })));
      expect(await screen.findByRole("button", { name: "Stop" })).toBeInTheDocument();
    });
  });
});
