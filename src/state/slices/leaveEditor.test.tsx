import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { Dialogs } from "../../components/Dialogs";
import { at } from "../../test/dayFixtures";
import { recording } from "../../test/recordingFixtures";
import { resetStore, useStore } from "../store";

const mocked = vi.mocked(api, true);
const state = () => useStore.getState();
const selection = { start: at("09:00"), end: at("09:30"), blockIds: [] };

/** Opens a new-recording editor with the selection in the Week view. */
async function openEditor() {
  resetStore({ view: "week", day: "2026-03-10" });
  render(<Dialogs />);
  act(() => {
    state().setSelection(selection);
    state().openEditor({ mode: "create", start: selection.start, end: selection.end });
  });
  await vi.waitFor(() => expect(state().draft).not.toBeNull());
}

/** Opens the editor on the existing recording "r1" (the selection's span) in the Week view and waits for its draft. */
async function openEdited() {
  mocked.recordingsList.mockResolvedValue([recording({ id: "r1", note: "standup", start: selection.start, end: selection.end })]);
  resetStore({ view: "week", day: "2026-03-10" });
  render(<Dialogs />);
  act(() => state().openEditor({ mode: "edit", id: "r1", at: { start: selection.start, end: selection.end } }));
  await vi.waitFor(() => expect(state().draft).not.toBeNull());
}

const longer = { start: selection.start, end: at("09:45") };
const otherRecording = { mode: "edit", id: "r2", at: { start: at("11:00"), end: at("11:45") } } as const;
const discardDialog = () => screen.queryByRole("heading", { name: "Discard changes?" });

describe("leaving a view with the recording editor open", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.recordingsList.mockResolvedValue([]);
  });

  describe("without an editor", () => {
    it("setView switches the view and clears the selection", () => {
      resetStore({ view: "day", selection, selectionSnap: selection });
      state().setView("week");
      expect(state().view).toBe("week");
      expect([state().selection, state().selectionSnap]).toEqual([null, null]);
    });

    it("goToDay shows the day in the Day view and clears the selection", () => {
      resetStore({ view: "month", selection, selectionSnap: selection });
      state().goToDay("2026-03-12");
      expect([state().view, state().day, state().selection]).toEqual(["day", "2026-03-12", null]);
    });

    it("runs the callback once the view is switched", () => {
      resetStore({ view: "day" });
      const viewSeen: string[] = [];
      state().setView("week", () => viewSeen.push(state().view));
      expect(viewSeen).toEqual(["week"]);
    });
  });

  describe("with an unchanged editor", () => {
    it("setView closes the editor and switches without asking", async () => {
      await openEditor();
      state().setView("month");
      expect(state().view).toBe("month");
      expect([state().editor, state().draft, state().selection]).toEqual([null, null, null]);
      expect(discardDialog()).not.toBeInTheDocument();
    });

    it("goToDay closes the editor and shows the day in the Day view", async () => {
      await openEditor();
      state().goToDay("2026-03-12");
      expect([state().view, state().day, state().editor]).toEqual(["day", "2026-03-12", null]);
    });

    it("showHours closes the editor and shows today in the Day view", async () => {
      await openEditor();
      state().showHours();
      expect([state().view, state().editor]).toEqual(["day", null]);
    });

    it("counts a picked project as a change", async () => {
      await openEditor();
      act(() => state().patchDraft({ projectId: "p-web" }));
      act(() => state().setView("month"));
      expect(screen.getByRole("heading", { name: "Discard changes?" })).toBeInTheDocument();
    });

    it("counts a new recording's changed times as a change", async () => {
      await openEditor();
      await act(() => state().setDraftSpan(0, longer));
      act(() => state().setView("month"));
      expect(screen.getByRole("heading", { name: "Discard changes?" })).toBeInTheDocument();
    });
  });

  describe("with an edited recording", () => {
    it("switches without asking after a time change, which is saved already", async () => {
      await openEdited();
      await act(() => state().setDraftSpan(0, longer));
      act(() => state().setView("month"));
      expect(discardDialog()).not.toBeInTheDocument();
      expect([state().view, state().editor]).toEqual(["month", null]);
    });

    it("asks when the note changed", async () => {
      await openEdited();
      act(() => state().patchDraft({ note: "standup, moved" }));
      act(() => state().setView("month"));
      expect(screen.getByRole("heading", { name: "Discard changes?" })).toBeInTheDocument();
      expect(state().view).toBe("week");
    });
  });

  describe("switching to another recording", () => {
    it("opens it at once when the open editor is unchanged", async () => {
      await openEditor();
      act(() => state().switchEditor(otherRecording));
      expect(discardDialog()).not.toBeInTheDocument();
      expect(state().editor).toEqual(otherRecording);
    });

    it("opens it at once after a time change of an edited recording", async () => {
      await openEdited();
      await act(() => state().setDraftSpan(0, longer));
      act(() => state().switchEditor(otherRecording));
      expect(discardDialog()).not.toBeInTheDocument();
      expect(state().editor).toEqual(otherRecording);
    });

    it.each([
      ["a new recording's draft", openEditor],
      ["an edited recording's fields", openEdited],
    ])("asks first when %s has unsaved changes, and Keep editing stays", async (_name, open) => {
      await open();
      act(() => state().patchDraft({ note: "half written" }));
      act(() => state().switchEditor(otherRecording));
      expect(screen.getByRole("heading", { name: "Discard changes?" })).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Keep editing" }));
      expect(state().editor).not.toEqual(otherRecording);
      expect(state().draft?.note).toBe("half written");
    });

    it.each([
      ["a new recording's draft", openEditor],
      ["an edited recording's fields", openEdited],
    ])("opens it after Discard of %s", async (_name, open) => {
      await open();
      act(() => state().patchDraft({ note: "half written" }));
      act(() => state().switchEditor(otherRecording));
      await userEvent.click(screen.getByRole("button", { name: "Discard" }));
      expect(state().editor).toEqual(otherRecording);
    });

    it("keeps the recording that is already open with its typed note", async () => {
      await openEdited();
      act(() => state().patchDraft({ note: "typed" }));
      act(() => state().switchEditor({ mode: "edit", id: "r1" }));
      expect(discardDialog()).not.toBeInTheDocument();
      expect(state().editor).toEqual({ mode: "edit", id: "r1", at: { start: selection.start, end: selection.end } });
      expect(state().draft?.note).toBe("typed");
    });

    describe("from a suggestion", () => {
      const suggested = { mode: "create", start: selection.start, end: selection.end, suggestion: { id: "s1", by: "Planner" } } as const;

      async function openSuggestion() {
        resetStore({ view: "day", day: "2026-03-10" });
        render(<Dialogs />);
        act(() => state().openEditor(suggested));
        await vi.waitFor(() => expect(state().draft).not.toBeNull());
        act(() => state().patchDraft({ note: "typed" }));
      }

      it("keeps the same suggestion open with its typed note", async () => {
        await openSuggestion();
        act(() => state().switchEditor(suggested));
        expect(discardDialog()).not.toBeInTheDocument();
        expect(state().draft?.note).toBe("typed");
      });

      it("asks before opening another suggestion", async () => {
        await openSuggestion();
        act(() => state().switchEditor({ ...suggested, suggestion: { id: "s2", by: "Planner" } }));
        expect(screen.getByRole("heading", { name: "Discard changes?" })).toBeInTheDocument();
        expect(state().editor).toEqual(suggested);
      });
    });
  });

  describe("with a changed draft", () => {
    beforeEach(async () => {
      await openEditor();
      act(() => state().patchDraft({ note: "half written" }));
    });

    it("asks before switching and keeps the view and the editor meanwhile", () => {
      act(() => state().setView("month"));
      expect(screen.getByRole("heading", { name: "Discard changes?" })).toBeInTheDocument();
      expect(state().view).toBe("week");
      expect(state().editor).not.toBeNull();
    });

    it("Discard closes the editor, switches the view and runs the callback", async () => {
      const then = vi.fn();
      act(() => state().setView("month", then));
      await userEvent.click(screen.getByRole("button", { name: "Discard" }));
      expect(state().view).toBe("month");
      expect([state().editor, state().draft]).toEqual([null, null]);
      expect(then).toHaveBeenCalled();
    });

    it("Discard on goToDay shows the day in the Day view", async () => {
      act(() => state().goToDay("2026-03-12"));
      await userEvent.click(screen.getByRole("button", { name: "Discard" }));
      expect([state().view, state().day, state().editor]).toEqual(["day", "2026-03-12", null]);
    });

    it("Keep editing stays on the view with the editor and never runs the callback", async () => {
      const then = vi.fn();
      act(() => state().setView("month", then));
      await userEvent.click(screen.getByRole("button", { name: "Keep editing" }));
      expect(discardDialog()).not.toBeInTheDocument();
      expect(state().view).toBe("week");
      expect(state().draft?.note).toBe("half written");
      expect(then).not.toHaveBeenCalled();
    });

    it("does not ask when the view stays the same", () => {
      act(() => state().setView("week"));
      expect(discardDialog()).not.toBeInTheDocument();
      expect(state().editor).not.toBeNull();
    });

    describe("revealEntry", () => {
      const entry = { day: "2026-03-12", kind: "suggestion", id: "s1" } as const;

      it("asks first and keeps the view, the day and the editor meanwhile", () => {
        act(() => state().revealEntry(entry));
        expect(screen.getByRole("heading", { name: "Discard changes?" })).toBeInTheDocument();
        expect([state().view, state().day, state().revealing]).toEqual(["week", "2026-03-10", null]);
        expect(state().editor).not.toBeNull();
      });

      it("Discard closes the editor and shows the entry's day in the Day view", async () => {
        act(() => state().revealEntry(entry));
        await userEvent.click(screen.getByRole("button", { name: "Discard" }));
        expect([state().view, state().day, state().editor, state().revealing]).toEqual(["day", "2026-03-12", null, entry]);
      });

      it("Keep editing leaves everything as it was", async () => {
        act(() => state().revealEntry(entry));
        await userEvent.click(screen.getByRole("button", { name: "Keep editing" }));
        expect([state().view, state().day, state().revealing]).toEqual(["week", "2026-03-10", null]);
        expect(state().draft?.note).toBe("half written");
      });
    });
  });

  describe("revealEntry without an editor", () => {
    it("shows the entry's day in the Day view, clears the selection and notes the entry", () => {
      const entry = { day: "2026-03-12", kind: "recording", id: "r1" } as const;
      resetStore({ view: "month", day: "2026-03-10", selection, selectionSnap: selection });
      state().revealEntry(entry);
      expect([state().view, state().day, state().selection, state().revealing]).toEqual(["day", "2026-03-12", null, entry]);
    });
  });
});
