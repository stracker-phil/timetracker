import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { formatDayLabel } from "../../lib/day";
import { resetStore, useStore } from "../../state/store";
import { at, DAY, dayBounds } from "../../test/dayFixtures";
import { Dialogs } from "../Dialogs";
import { Toasts } from "../Toasts";

const mocked = vi.mocked(api, true);

const bounds = dayBounds(DAY);
const label = formatDayLabel(DAY);

function renderDialog(range = false) {
  useStore.getState().openDialog({ type: "deleteActivity", day: DAY, bounds, range });
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
    mocked.activityCount.mockResolvedValue({ activities: 12, away: 1 });
    mocked.activityDelete.mockResolvedValue({ activities: 12, away: 1, undoUntil: Date.now() + 30_000 });
  });

  afterEach(() => vi.useRealTimers());

  describe("whole day", () => {
    it("shows the day and how many spans the day holds", async () => {
      renderDialog();
      expect(screen.getByRole("heading", { name: `Delete activity for ${label}` })).toBeInTheDocument();
      expect(await screen.findByText(/Deletes 12 activity spans and 1 away span\./)).toBeInTheDocument();
      expect(mocked.activityCount).toHaveBeenCalledWith(bounds[0], bounds[1]);
    });

    it("keeps Delete disabled until the count is known", async () => {
      renderDialog();
      expect(deleteButton()).toBeDisabled();
      await screen.findByText(/Deletes 12 activity spans/);
      expect(deleteButton()).toBeEnabled();
    });

    it("deletes the day's bounds, closes and offers Undo in a toast", async () => {
      renderDialog();
      await screen.findByText(/Deletes 12 activity spans/);
      await userEvent.click(deleteButton());

      expect(mocked.activityDelete).toHaveBeenCalledWith(bounds[0], bounds[1]);
      expect(await screen.findByText(`Deleted activity for ${label}`)).toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
    });

    it("cancels without deleting", async () => {
      renderDialog();
      await screen.findByText(/Deletes 12 activity spans/);
      await userEvent.click(screen.getByRole("button", { name: "Cancel" }));

      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mocked.activityDelete).not.toHaveBeenCalled();
    });

    it("shows the error of a failed delete and keeps the dialog open", async () => {
      mocked.activityDelete.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      renderDialog();
      await screen.findByText(/Deletes 12 activity spans/);
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

    it("counts a single span in the singular", async () => {
      mocked.activityCount.mockResolvedValue({ activities: 1, away: 0 });
      renderDialog();
      expect(await screen.findByText(/Deletes 1 activity span and 0 away spans\./)).toBeInTheDocument();
    });
  });

  describe("undo", () => {
    async function deleteDay() {
      renderDialog();
      await screen.findByText(/Deletes 12 activity spans/);
      await userEvent.click(deleteButton());
      await userEvent.click(await screen.findByRole("button", { name: "Undo" }));
    }

    it("restores the spans and confirms the count", async () => {
      mocked.activityDeleteUndo.mockResolvedValue({ activities: 12, away: 1, undoUntil: 0 });
      await deleteDay();

      expect(await screen.findByText("Restored 12 activity spans and 1 away span")).toBeInTheDocument();
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
      await screen.findByText(/Deletes 12 activity spans/);
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
      await screen.findByText(/Deletes 12 activity spans/);
    });

    it("recounts and deletes the edited range", async () => {
      renderDialog(true);
      await screen.findByText(/Deletes 12 activity spans/);
      mocked.activityCount.mockResolvedValue({ activities: 3, away: 0 });
      fireEvent.change(screen.getByLabelText("From"), { target: { value: `${DAY}T10:00` } });
      fireEvent.change(screen.getByLabelText("To"), { target: { value: `${DAY}T11:30` } });

      expect(await screen.findByText(/Deletes 3 activity spans and 0 away spans\./)).toBeInTheDocument();
      expect(mocked.activityCount).toHaveBeenLastCalledWith(at("10:00"), at("11:30"));

      await userEvent.click(deleteButton());
      expect(mocked.activityDelete).toHaveBeenCalledWith(at("10:00"), at("11:30"));
      expect(await screen.findByText(/^Deleted activity for .*10:00 – .*11:30$/)).toBeInTheDocument();
    });

    it("disables Delete while a field is empty", async () => {
      renderDialog(true);
      await screen.findByText(/Deletes 12 activity spans/);
      fireEvent.change(screen.getByLabelText("From"), { target: { value: "" } });

      expect(deleteButton()).toBeDisabled();
    });
  });
});
