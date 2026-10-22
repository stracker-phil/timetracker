import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type AwayPrompt as Prompt } from "../../api";
import { resetStore } from "../../state/store";
import { at } from "../../test/dayFixtures";
import { awayPrompt } from "../../test/listFixtures";
import { projectItem } from "../../test/projectFixtures";
import { recording } from "../../test/recordingFixtures";
import { AwayPrompt } from "./AwayPrompt";

const mocked = vi.mocked(api, true);

const website = projectItem({ project: { id: "p-site", name: "Website" } });

const field = (label: string) => screen.getByLabelText(label);
const recordButton = () => screen.getByRole("button", { name: "Record" });

/** Renders the window and waits until the form and the project list are there. */
async function renderPrompt() {
  resetStore();
  render(<AwayPrompt />);
  await screen.findAllByRole("option", { name: "Website" });
}

/** Delivers an `away_prompt_changed` event to the window. */
function emitChanged(prompt: Prompt) {
  act(() => void mocked.onAwayPromptChanged.mock.calls[0][0](prompt));
}

describe("AwayPrompt", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.awayPromptCurrent.mockResolvedValue(awayPrompt());
    mocked.projectsList.mockResolvedValue([website]);
    mocked.projectsRecent.mockResolvedValue([website.project]);
  });

  describe("the prompt", () => {
    it("shows how long the user was away and when", async () => {
      await renderPrompt();
      expect(screen.getByRole("heading", { name: "You were away 42 min" })).toBeInTheDocument();
      expect(screen.getByText("12:03–12:45 · idle")).toBeInTheDocument();
    });

    it.each([
      ["sleep", "asleep"],
      ["lock", "locked"],
      ["paused", "paused"],
    ] as const)("names the reason %s as %s", async (reason, word) => {
      mocked.awayPromptCurrent.mockResolvedValue(awayPrompt({ span: { id: 1, start: at("12:03"), end: at("12:45"), reason } }));
      await renderPrompt();
      expect(screen.getByText(`12:03–12:45 · ${word}`)).toBeInTheDocument();
    });

    it("fills the times with the suggestion", async () => {
      await renderPrompt();
      expect(field("From")).toHaveValue("12:05");
      expect(field("To")).toHaveValue("12:45");
    });

    it("takes the suggested end for the length of a span that is still open", async () => {
      mocked.awayPromptCurrent.mockResolvedValue(awayPrompt({ span: { id: 1, start: at("12:03"), end: null, reason: "idle" } }));
      await renderPrompt();
      expect(screen.getByRole("heading", { name: "You were away 42 min" })).toBeInTheDocument();
    });

    it("says there is nothing to record when no prompt is open, and Close skips", async () => {
      mocked.awayPromptCurrent.mockResolvedValue(null);
      resetStore();
      render(<AwayPrompt />);
      expect(await screen.findByText("Nothing to record")).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Close" }));
      expect(mocked.awayPromptSkip).toHaveBeenCalled();
    });

    it("says there is nothing to record when the prompt cannot be loaded", async () => {
      mocked.awayPromptCurrent.mockRejectedValue({ kind: "io", message: "Channel closed", code: null });
      resetStore();
      render(<AwayPrompt />);
      expect(await screen.findByText("Nothing to record")).toBeInTheDocument();
    });
  });

  describe("recording", () => {
    it("answers with the last used project, the suggested times and the trimmed note on Enter", async () => {
      await renderPrompt();
      await userEvent.type(field("Note"), "  lunch {Enter}");
      expect(mocked.awayPromptAnswer).toHaveBeenCalledWith({ projectId: "p-site", spans: [{ start: at("12:05"), end: at("12:45") }], note: "lunch", tagId: null });
    });

    it("answers with the edited times and the chosen tag when Record is clicked", async () => {
      await renderPrompt();
      await userEvent.clear(field("From"));
      await userEvent.type(field("From"), "12:10");
      await userEvent.clear(field("To"));
      await userEvent.type(field("To"), "12:40");
      await userEvent.type(screen.getByRole("combobox", { name: "Tag" }), "res{Enter}");
      await userEvent.click(recordButton());
      expect(mocked.awayPromptAnswer).toHaveBeenCalledWith({ projectId: "p-site", spans: [{ start: at("12:10"), end: at("12:40") }], note: "", tagId: "tag-research" });
    });

    it("takes an end before the start as the next day", async () => {
      await renderPrompt();
      await userEvent.clear(field("To"));
      await userEvent.type(field("To"), "08:00");
      await userEvent.click(recordButton());
      expect(mocked.awayPromptAnswer).toHaveBeenCalledWith(expect.objectContaining({ spans: [{ start: at("12:05"), end: at("08:00", "2026-03-11") }] }));
    });

    it("asks for a project and answers nothing while none is known", async () => {
      mocked.projectsRecent.mockResolvedValue([]);
      await renderPrompt();
      await userEvent.click(recordButton());
      expect(await screen.findByRole("alert")).toHaveTextContent("Pick a project");
      expect(mocked.awayPromptAnswer).not.toHaveBeenCalled();
    });

    it("answers with the project picked instead of the last used one", async () => {
      const docs = projectItem({ project: { id: "p-docs", name: "Docs" } });
      mocked.projectsList.mockResolvedValue([website, docs]);
      await renderPrompt();
      await userEvent.click(screen.getAllByRole("option", { name: "Docs" })[0]);
      await userEvent.click(recordButton());
      expect(mocked.awayPromptAnswer).toHaveBeenCalledWith(expect.objectContaining({ projectId: "p-docs" }));
    });
  });

  describe("skipping", () => {
    it("skips with the Skip button", async () => {
      await renderPrompt();
      await userEvent.click(screen.getByRole("button", { name: "Skip" }));
      expect(mocked.awayPromptSkip).toHaveBeenCalled();
      expect(mocked.awayPromptAnswer).not.toHaveBeenCalled();
    });

    it("skips with Escape", async () => {
      await renderPrompt();
      await userEvent.click(field("Note"));
      await userEvent.keyboard("{Escape}");
      expect(mocked.awayPromptSkip).toHaveBeenCalled();
    });
  });

  describe("errors", () => {
    const inTheWay = recording({ id: "r9", projectId: "p-site", start: at("12:10"), end: at("12:30") });

    it("shows the recording in the way inline and keeps the form", async () => {
      mocked.awayPromptAnswer.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps", code: null, details: [inTheWay] });
      await renderPrompt();
      await userEvent.type(field("Note"), "lunch");
      await userEvent.click(recordButton());
      expect(await screen.findByRole("alert")).toHaveTextContent("A recording already covers 12:10–12:30 (Website)");
      expect(field("Note")).toHaveValue("lunch");
      expect(recordButton()).toBeEnabled();
      expect(mocked.awayPromptSkip).not.toHaveBeenCalled();
    });

    it("lists every recording in the way", async () => {
      const later = recording({ id: "r10", projectId: "p-site", start: at("12:35"), end: at("12:40") });
      mocked.awayPromptAnswer.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps", code: null, details: [inTheWay, later] });
      await renderPrompt();
      await userEvent.click(recordButton());
      expect(await screen.findByRole("alert")).toHaveTextContent("Recordings already cover 12:10–12:30 (Website), 12:35–12:40 (Website)");
    });

    it("lists every span of a recording in the way", async () => {
      const sittings = recording({
        id: "r11",
        projectId: "p-site",
        spans: [
          { id: "r11-s1", start: at("12:10"), end: at("12:20") },
          { id: "r11-s2", start: at("12:30"), end: at("12:40") },
        ],
      });
      mocked.awayPromptAnswer.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps", code: null, details: [sittings] });
      await renderPrompt();
      await userEvent.click(recordButton());
      expect(await screen.findByRole("alert")).toHaveTextContent("A recording already covers 12:10–12:20, 12:30–12:40 (Website)");
    });

    it("answers again with the adjusted times after an overlap", async () => {
      mocked.awayPromptAnswer.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps", code: null, details: [inTheWay] });
      await renderPrompt();
      await userEvent.click(recordButton());
      await screen.findByRole("alert");
      await userEvent.clear(field("From"));
      await userEvent.type(field("From"), "12:30");
      await userEvent.click(recordButton());
      expect(mocked.awayPromptAnswer).toHaveBeenLastCalledWith(expect.objectContaining({ spans: [{ start: at("12:30"), end: at("12:45") }] }));
    });

    it.each([
      ["note", "The note is too long", "Note"],
      ["end", "The end is too late", "To"],
    ])("shows the %s error of Rust under its field", async (path, message) => {
      mocked.awayPromptAnswer.mockRejectedValueOnce({ kind: "invalid_input", message: "Invalid", code: null, field: { path, message } });
      await renderPrompt();
      await userEvent.click(recordButton());
      expect(await screen.findByText(message)).toBeInTheDocument();
    });

    it("shows an error that belongs to no field inline", async () => {
      mocked.awayPromptAnswer.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      await renderPrompt();
      await userEvent.click(recordButton());
      expect(await screen.findByRole("alert")).toHaveTextContent("Database locked");
    });

    it("reloads the prompt when it is gone", async () => {
      mocked.awayPromptAnswer.mockRejectedValueOnce({ kind: "not_found", message: "No away prompt is open", code: null });
      await renderPrompt();
      mocked.awayPromptCurrent.mockResolvedValue(null);
      await userEvent.click(recordButton());
      expect(await screen.findByText("Nothing to record")).toBeInTheDocument();
    });
  });

  describe("a newer away span", () => {
    const newer = awayPrompt({
      span: { id: 2, start: at("15:00"), end: at("15:20"), reason: "lock" },
      suggested: { start: at("15:00"), end: at("15:20") },
    });

    it("refills the form from an away_prompt_changed event", async () => {
      await renderPrompt();
      await userEvent.type(field("Note"), "lunch");
      emitChanged(newer);
      expect(await screen.findByRole("heading", { name: "You were away 20 min" })).toBeInTheDocument();
      expect(screen.getByText("15:00–15:20 · locked")).toBeInTheDocument();
      expect(field("From")).toHaveValue("15:00");
      expect(field("To")).toHaveValue("15:20");
      expect(field("Note")).toHaveValue("");
    });

    it("answers with the times of the newer span", async () => {
      await renderPrompt();
      emitChanged(newer);
      await screen.findByRole("heading", { name: "You were away 20 min" });
      await userEvent.click(recordButton());
      expect(mocked.awayPromptAnswer).toHaveBeenCalledWith(expect.objectContaining({ spans: [{ start: at("15:00"), end: at("15:20") }] }));
    });

    it("reloads the prompt when the window gains focus", async () => {
      await renderPrompt();
      mocked.awayPromptCurrent.mockResolvedValue(newer);
      act(() => void window.dispatchEvent(new Event("focus")));
      expect(await screen.findByRole("heading", { name: "You were away 20 min" })).toBeInTheDocument();
    });
  });
});
