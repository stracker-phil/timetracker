import { act, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type EntryRef } from "../../api";
import { installCommands } from "../../commands";
import { renderWithClient, seedProjects, seedQuery, SETTINGS_KEY, TRACKER_KEY } from "../../test/queries";
import { at, DAY, dayView, recordingBlock, withTimesheet } from "../../test/dayFixtures";
import { suggestion } from "../../test/ruleFixtures";
import { settings } from "../../test/settings";
import { trackerStatus } from "../../test/data";
import { resetStore, useStore } from "../../state/store";
import { DayPage } from "./DayPage";

const mocked = vi.mocked(api, true);

const OTHER_DAY = "2026-03-12";

const callRecording = (day: string, id = "r1") =>
  recordingBlock({ id, projectName: "Website relaunch", note: "weekly call", start: at("09:00", day), end: at("10:00", day) });
const designReview = (day: string) => suggestion({ projectName: "Website relaunch", note: "design review", by: "Planner", start: at("14:00", day), end: at("14:30", day) });

function loadDays(build: (day: string) => ReturnType<typeof dayView>) {
  mocked.dayView.mockImplementation(async (day) => build(day));
}

function mountDay(day: string) {
  resetStore({ view: "day", day, permissionDialogHidden: true });
  seedQuery(SETTINGS_KEY, settings());
  seedQuery(TRACKER_KEY, trackerStatus());
  seedProjects({ projects: [] });
  installCommands();
  return renderWithClient(<DayPage />);
}

const reveal = (entry: EntryRef) => act(() => useStore.getState().revealEntry(entry));
const recordingCard = () => screen.findByRole("button", { name: /^weekly call, Website relaunch/ });
const suggestionCard = () => screen.findByRole("group", { name: /^Suggested: design review/ });

describe("DayPage revealing an entry", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.recordingsList.mockResolvedValue([]);
  });

  describe("on the day that is shown already", () => {
    beforeEach(() => {
      const retro = (day: string) => recordingBlock({ id: "r2", projectName: "Website relaunch", note: "retro", start: at("11:00", day), end: at("11:30", day) });
      loadDays((day) => withTimesheet(dayView({ day, recordings: [callRecording(day), retro(day)], suggestions: [designReview(day)] })));
    });

    it("focuses the recording's card in the timesheet", async () => {
      mountDay(DAY);
      expect(await recordingCard()).toHaveAttribute("aria-pressed", "false");
      reveal({ day: DAY, kind: "recording", id: "r1" });
      expect(await recordingCard()).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: /^retro, / })).toHaveAttribute("aria-pressed", "false");
    });

    it("focuses the suggestion's card", async () => {
      const idea = designReview(DAY);
      mountDay(DAY);
      expect(await suggestionCard()).not.toHaveClass("focused");
      reveal({ day: DAY, kind: "suggestion", id: idea.id });
      expect(await suggestionCard()).toHaveClass("focused");
    });

    it("moves the focus from a focused recording to the suggestion", async () => {
      const idea = designReview(DAY);
      mountDay(DAY);
      await recordingCard();
      act(() => useStore.getState().focusRecording("r2"));
      reveal({ day: DAY, kind: "suggestion", id: idea.id });
      expect(await suggestionCard()).toHaveClass("focused");
      expect(screen.getByRole("button", { name: /^retro, / })).toHaveAttribute("aria-pressed", "false");
    });

    it("moves the focus from a focused suggestion to the recording", async () => {
      const idea = designReview(DAY);
      mountDay(DAY);
      await recordingCard();
      act(() => useStore.getState().focusSuggestion(idea.id));
      expect(await suggestionCard()).toHaveClass("focused");
      reveal({ day: DAY, kind: "recording", id: "r1" });
      expect(await recordingCard()).toHaveAttribute("aria-pressed", "true");
      expect(await suggestionCard()).not.toHaveClass("focused");
    });

    it("is done with the entry once it is focused", async () => {
      mountDay(DAY);
      await recordingCard();
      reveal({ day: DAY, kind: "recording", id: "r1" });
      await vi.waitFor(() => expect(useStore.getState().revealing).toBeNull());
    });
  });

  describe("on a day that has to load", () => {
    beforeEach(() => {
      loadDays((day) => withTimesheet(dayView({ day, recordings: day === OTHER_DAY ? [callRecording(day)] : [], suggestions: day === OTHER_DAY ? [designReview(day)] : [] })));
    });

    it("shows the day and focuses the recording once it has loaded", async () => {
      mountDay(DAY);
      reveal({ day: OTHER_DAY, kind: "recording", id: "r1" });
      expect(await recordingCard()).toHaveAttribute("aria-pressed", "true");
      expect(useStore.getState().day).toBe(OTHER_DAY);
      expect(useStore.getState().revealing).toBeNull();
    });

    it("focuses the suggestion once the day has loaded", async () => {
      const idea = designReview(OTHER_DAY);
      mountDay(DAY);
      reveal({ day: OTHER_DAY, kind: "suggestion", id: idea.id });
      expect(await suggestionCard()).toHaveClass("focused");
    });

    it("focuses nothing when the day is shown without a reveal", async () => {
      mountDay(DAY);
      act(() => useStore.getState().goToDay(OTHER_DAY));
      expect(await recordingCard()).toHaveAttribute("aria-pressed", "false");
      expect(await suggestionCard()).not.toHaveClass("focused");
    });
  });
});
