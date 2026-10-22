import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { at, DAY, dayBounds } from "../../test/dayFixtures";
import { Dialogs } from "../Dialogs";
import { Toasts } from "../Toasts";

const mocked = vi.mocked(api, true);

const bounds = dayBounds(DAY);
const label = "Wednesday, October 21, 2026";
const rangeLabel = "Wed, Oct 21 10:00 – Wed, Oct 21 11:30";

function renderDialog(range = false) {
  useStore.getState().openDialog({ type: "deleteActivity", day: DAY, label, bounds, range });
  return render(
    <>
      <Dialogs />
      <Toasts />
    </>,
  );
}

const deleteButton = () => screen.getByRole("button", { name: "Delete" });

describe("DeleteActivityDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    mocked.activityCount.mockResolvedValue({ activities: 12, away: 1, rangeLabel });
    mocked.activityDelete.mockResolvedValue({ activities: 12, away: 1, undoUntil: Date.now() + 30_000 });
  });

  afterEach(() => {
    vi.useRealTimers();
    mocked.calendarParseTime.mockReset();
  });

  describe("whole day", () => {
    it("shows the day and how much activity the day holds", async () => {
      renderDialog();
      expect(screen.getByRole("heading", { name: "Delete activity for Wednesday, October 21, 2026" })).toBeInTheDocument();
      expect(await screen.findByText(/Deletes 12 activities and 1 away time\./)).toBeInTheDocument();
      expect(screen.getByText(/Activity and away time that cross the range are deleted entirely\./)).toBeInTheDocument();
      expect(mocked.activityCount).toHaveBeenCalledWith(bounds[0], bounds[1]);
    });

    it("keeps Delete disabled until the count is known", async () => {
      renderDialog();
      expect(deleteButton()).toBeDisabled();
      await screen.findByText(/Deletes 12 activities/);
      expect(deleteButton()).toBeEnabled();
    });

    it("deletes the day's bounds, closes and offers Undo in a toast", async () => {
      renderDialog();
      await screen.findByText(/Deletes 12 activities/);
      await userEvent.click(deleteButton());

      expect(mocked.activityDelete).toHaveBeenCalledWith(bounds[0], bounds[1]);
      expect(await screen.findByText(`Deleted activity for ${label}`)).toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
    });

    it("cancels without deleting", async () => {
      renderDialog();
      await screen.findByText(/Deletes 12 activities/);
      await userEvent.click(screen.getByRole("button", { name: "Cancel" }));

      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mocked.activityDelete).not.toHaveBeenCalled();
    });

    it("shows the error of a failed delete and keeps the dialog open", async () => {
      mocked.activityDelete.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      renderDialog();
      await screen.findByText(/Deletes 12 activities/);
      await userEvent.click(deleteButton());

      expect(await screen.findByRole("alert")).toHaveTextContent("Disk full");
      expect(screen.getByRole("dialog")).toBeInTheDocument();
      expect(deleteButton()).toBeDisabled();
    });

    it("shows the error of a failed count and disables Delete", async () => {
      mocked.activityCount.mockRejectedValue({ kind: "io", message: "Database locked", code: null });
      renderDialog();

      expect(await screen.findByRole("alert")).toHaveTextContent("Database locked");
      expect(screen.queryByText(/Deletes \d+/)).not.toBeInTheDocument();
      expect(deleteButton()).toBeDisabled();
    });

    it("counts a single activity in the singular", async () => {
      mocked.activityCount.mockResolvedValue({ activities: 1, away: 0, rangeLabel });
      renderDialog();
      expect(await screen.findByText(/Deletes 1 activity and 0 away times\./)).toBeInTheDocument();
    });
  });

  describe("undo", () => {
    async function deleteDay() {
      renderDialog();
      await screen.findByText(/Deletes 12 activities/);
      await userEvent.click(deleteButton());
      await userEvent.click(await screen.findByRole("button", { name: "Undo" }));
    }

    it("restores the activity and confirms the count", async () => {
      mocked.activityDeleteUndo.mockResolvedValue({ activities: 12, away: 1, undoUntil: 0 });
      await deleteDay();

      expect(await screen.findByText("Restored 12 activities and 1 away time")).toBeInTheDocument();
      expect(mocked.activityDeleteUndo).toHaveBeenCalled();
    });

    it("shows the error when the undo fails", async () => {
      mocked.activityDeleteUndo.mockRejectedValue({ kind: "not_found", message: "Nothing to undo", code: null });
      await deleteDay();

      expect(await screen.findByRole("alert")).toHaveTextContent("Couldn't undo the delete: Nothing to undo");
    });

    it("keeps the undo toast until the undo window ends, not the default 10 s", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      mocked.activityDelete.mockResolvedValue({ activities: 12, away: 1, undoUntil: Date.now() + 30_000 });
      renderDialog();
      await screen.findByText(/Deletes 12 activities/);
      await userEvent.click(deleteButton());
      await screen.findByText(`Deleted activity for ${label}`);

      await act(() => vi.advanceTimersByTimeAsync(20_000));
      expect(screen.getByText(`Deleted activity for ${label}`)).toBeInTheDocument();

      await act(() => vi.advanceTimersByTimeAsync(11_000));
      expect(screen.queryByText(`Deleted activity for ${label}`)).not.toBeInTheDocument();
    });
  });

  describe("range", () => {
    it("starts with the day's bounds in the From and To fields", async () => {
      renderDialog(true);
      expect(screen.getByRole("heading", { name: "Delete activity in a range" })).toBeInTheDocument();
      expect(screen.getByLabelText("From")).toHaveValue(`${DAY}T00:00`);
      expect(screen.getByLabelText("To")).toHaveValue("2026-03-11T00:00");
      await screen.findByText(/Deletes 12 activities/);
    });

    it("recounts and deletes the edited range", async () => {
      renderDialog(true);
      await screen.findByText(/Deletes 12 activities/);
      mocked.activityCount.mockResolvedValue({ activities: 3, away: 0, rangeLabel });
      fireEvent.change(screen.getByLabelText("From"), { target: { value: `${DAY}T10:00` } });
      fireEvent.change(screen.getByLabelText("To"), { target: { value: `${DAY}T11:30` } });

      expect(await screen.findByText(/Deletes 3 activities and 0 away times\./)).toBeInTheDocument();
      expect(mocked.activityCount).toHaveBeenLastCalledWith(at("10:00"), at("11:30"));

      await userEvent.click(deleteButton());
      expect(mocked.activityDelete).toHaveBeenCalledWith(at("10:00"), at("11:30"));
      expect(await screen.findByText(`Deleted activity for ${rangeLabel}`)).toBeInTheDocument();
    });

    it("counts and deletes the range Rust read from the fields, not the typed text", async () => {
      const reading: Record<string, number> = {
        [`${DAY}T00:00`]: bounds[0],
        "2026-03-11T00:00": bounds[1],
        [`${DAY}T10:00`]: at("10:15"),
        [`${DAY}T11:30`]: at("11:45"),
      };
      mocked.calendarParseTime.mockImplementation(async (text) => reading[text] ?? null);
      renderDialog(true);
      await screen.findByText(/Deletes 12 activities/);
      fireEvent.change(screen.getByLabelText("From"), { target: { value: `${DAY}T10:00` } });
      fireEvent.change(screen.getByLabelText("To"), { target: { value: `${DAY}T11:30` } });

      await vi.waitFor(() => expect(mocked.activityCount).toHaveBeenLastCalledWith(at("10:15"), at("11:45")));
      expect(mocked.calendarParseTime).toHaveBeenCalledWith(`${DAY}T10:00`, bounds[0]);
      await userEvent.click(deleteButton());
      await vi.waitFor(() => expect(mocked.activityDelete).toHaveBeenCalledWith(at("10:15"), at("11:45")));
    });

    it("disables Delete while Rust reads a field as no time", async () => {
      mocked.calendarParseTime.mockImplementation(async (text) => (text.endsWith("T10:00") ? null : bounds[0]));
      renderDialog(true);
      await screen.findByText(/Deletes 12 activities/);
      await vi.waitFor(() => expect(deleteButton()).toBeEnabled());
      fireEvent.change(screen.getByLabelText("To"), { target: { value: "2026-03-11T10:00" } });

      await vi.waitFor(() => expect(deleteButton()).toBeDisabled());
    });

    it("disables Delete while a field is empty", async () => {
      renderDialog(true);
      await screen.findByText(/Deletes 12 activities/);
      fireEvent.change(screen.getByLabelText("From"), { target: { value: "" } });

      expect(deleteButton()).toBeDisabled();
    });
  });
});
