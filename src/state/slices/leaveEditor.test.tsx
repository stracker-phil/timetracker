import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { Dialogs } from "../../components/Dialogs";
import { at } from "../../test/dayFixtures";
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

    it("does not count a project the editor prefilled itself as a change", async () => {
      await openEditor();
      act(() => state().prefillDraft({ projectId: "p-site" }));
      state().setView("month");
      expect(state().view).toBe("month");
      expect(discardDialog()).not.toBeInTheDocument();
    });

    it("counts a project chosen after the prefill as a change", async () => {
      await openEditor();
      act(() => state().prefillDraft({ projectId: "p-site" }));
      act(() => state().patchDraft({ projectId: "p-web" }));
      act(() => state().setView("month"));
      expect(screen.getByRole("heading", { name: "Discard changes?" })).toBeInTheDocument();
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
  });
});
