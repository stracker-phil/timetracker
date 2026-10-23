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

/** Focuses the project field; typing opens the filtered list and ↵ picks the first match. */
async function chooseProject() {
  const field = await screen.findByRole("combobox", { name: "Project" });
  await vi.waitFor(() => expect(useStore.getState().projects.length).toBeGreaterThan(0));
  await userEvent.click(field);
  await userEvent.keyboard("Web{Enter}");
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
      expect(await screen.findByRole("heading", { name: "Start Timer" })).toBeInTheDocument();
      expect(startButton()).toBeDisabled();
      await chooseProject();
      expect(startButton()).toBeEnabled();
    });

    it("asks for Note, Project and Tag in that order, with the note focused and a prompt in it", async () => {
      renderPicker();
      const note = await screen.findByRole("textbox", { name: "Note" });
      const project = screen.getByRole("combobox", { name: "Project" });
      const tag = screen.getByRole("combobox", { name: "Tag" });
      expect(note).toHaveFocus();
      expect(note).toHaveAttribute("placeholder", "What did you work on?");
      expect(note.compareDocumentPosition(project) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
      expect(project.compareDocumentPosition(tag) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    });

    it("starts the timer with project, trimmed note and tag, then closes", async () => {
      renderPicker();
      await chooseProject();
      await userEvent.type(noteField(), "  standup ");
      await userEvent.click(screen.getByRole("combobox", { name: "Tag" }));
      await userEvent.keyboard("{ArrowDown}");
      await userEvent.click(screen.getByRole("option", { name: "support" }));
      await userEvent.click(startButton());
      expect(mocked.timerStart).toHaveBeenCalledWith("p-site", "standup", "tag-support");
      await vi.waitFor(() => expect(mocked.timerPickerClose).toHaveBeenCalled());
    });

    it("starts with Enter in the note field", async () => {
      renderPicker();
      await chooseProject();
      await userEvent.type(noteField(), "review{Enter}");
      expect(mocked.timerStart).toHaveBeenCalledWith("p-site", "review", null);
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

    it("starts with Enter in the project field while its list is closed", async () => {
      renderPicker();
      await chooseProject();
      await userEvent.keyboard("{Enter}");
      expect(mocked.timerStart).toHaveBeenCalledWith("p-site", "", null);
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

    it("starts a recent timer with its project, note and tag when clicked", async () => {
      mocked.timersRecent.mockResolvedValue([recentTimer({ projectId: "p-site", note: "standup", tagId: "tag-admin" }), recentTimer({ projectId: "p-docs", projectName: "Docs" })]);
      renderPicker();
      expect(await screen.findByRole("button", { name: "Docs" })).toBeInTheDocument();
      expect(screen.getByText("Recent")).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Website · standup" }));
      expect(mocked.timerStart).toHaveBeenCalledWith("p-site", "standup", "tag-admin");
      await vi.waitFor(() => expect(mocked.timerPickerClose).toHaveBeenCalled());
    });

    it("shows no recent list when there are no recent timers", async () => {
      renderPicker();
      await screen.findByRole("heading", { name: "Start Timer" });
      expect(screen.queryByRole("list", { name: "Recent timers" })).not.toBeInTheDocument();
      expect(screen.queryByText("Recent")).not.toBeInTheDocument();
    });
  });

  describe("with a running timer", () => {
    beforeEach(() => {
      mocked.timerStatus.mockResolvedValue(runningStatus({ projectName: "Website", recording: recording({ end: null, projectId: "p-site", note: "writing", tagId: "tag-support" }) }));
      mocked.timersRecent.mockResolvedValue([recentTimer()]);
    });

    it("shows the form filled with the running note, project and tag, and the elapsed time", async () => {
      renderPicker();
      expect(await screen.findByRole("heading", { level: 1, name: /Timer running/ })).toHaveTextContent("1h 23m");
      expect(noteField()).toHaveValue("writing");
      await vi.waitFor(() => expect(screen.getByRole("combobox", { name: "Project" })).toHaveDisplayValue("Website"));
      expect(screen.getByRole("combobox", { name: "Tag" })).toHaveDisplayValue("support");
      expect(screen.queryByRole("heading", { name: "Start Timer" })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Start another…" })).not.toBeInTheDocument();
    });

    it("hides the recent timers", async () => {
      renderPicker();
      await screen.findByRole("heading", { name: /Timer running/ });
      expect(screen.queryByRole("list", { name: "Recent timers" })).not.toBeInTheDocument();
      expect(screen.queryByText("Recent")).not.toBeInTheDocument();
    });

    it("saves the edited note with the unchanged project and tag, then closes", async () => {
      renderPicker();
      await screen.findByRole("heading", { name: /Timer running/ });
      await userEvent.clear(noteField());
      await userEvent.type(noteField(), "  reviewing ");
      await userEvent.click(screen.getByRole("button", { name: "Save" }));
      expect(mocked.timerUpdate).toHaveBeenCalledWith("p-site", "reviewing", "tag-support");
      expect(mocked.timerStart).not.toHaveBeenCalled();
      await vi.waitFor(() => expect(mocked.timerPickerClose).toHaveBeenCalled());
    });

    it("stays open when the timer cannot be changed", async () => {
      mocked.timerUpdate.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      renderPicker();
      await screen.findByRole("heading", { name: /Timer running/ });
      await userEvent.click(screen.getByRole("button", { name: "Save" }));
      await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message)).toContain("Could not change the timer: Disk full"));
      expect(mocked.timerPickerClose).not.toHaveBeenCalled();
    });

    it("stops the timer and closes on Stop", async () => {
      renderPicker();
      await userEvent.click(await screen.findByRole("button", { name: "Stop" }));
      expect(mocked.timerStop).toHaveBeenCalled();
      await vi.waitFor(() => expect(mocked.timerPickerClose).toHaveBeenCalled());
    });

    it("closes without saving on Cancel", async () => {
      renderPicker();
      await screen.findByRole("heading", { name: /Timer running/ });
      await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
      expect(mocked.timerPickerClose).toHaveBeenCalled();
      expect(mocked.timerUpdate).not.toHaveBeenCalled();
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
      await screen.findByRole("heading", { name: "Start Timer" });
      mocked.timerStatus.mockResolvedValue(runningStatus({ projectName: "Docs" }));
      act(() => void window.dispatchEvent(new Event("focus")));
      expect(await screen.findByRole("button", { name: "Stop" })).toBeInTheDocument();
    });

    it("follows timer status events", async () => {
      renderPicker();
      await screen.findByRole("heading", { name: "Start Timer" });
      act(() => mocked.onTimerStatus.mock.calls[0][0](runningStatus({ projectName: "Docs" })));
      expect(await screen.findByRole("button", { name: "Stop" })).toBeInTheDocument();
    });
  });
});
