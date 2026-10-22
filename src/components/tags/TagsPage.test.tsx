import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore } from "../../state/store";
import { projectsStore, tag } from "../../test/projectFixtures";
import { Dialogs } from "../Dialogs";
import { Toasts } from "../Toasts";
import { TagsPage } from "./TagsPage";

const mocked = vi.mocked(api, true);

function renderTags(tags = [tag({ name: "admin" }), tag({ name: "research", recordings: 3 }), tag({ name: "support", recordings: 1 })]) {
  resetStore(projectsStore({ tags }));
  render(
    <>
      <TagsPage />
      <Dialogs />
      <Toasts />
    </>,
  );
}

describe("TagsPage", () => {
  beforeEach(() => vi.clearAllMocks());

  it("is a page of its own titled Tags", () => {
    renderTags();
    expect(screen.getByRole("region", { name: "Tags" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Tags" })).toBeInTheDocument();
  });

  it("lists every tag with the number of recordings that carry it", () => {
    renderTags();
    const rows = screen.getAllByRole("listitem").map((r) => r.textContent);
    expect(rows).toEqual(["admin0 recordings", "research3 recordings", "support1 recording"]);
  });

  describe("deleting", () => {
    it("deletes an unused tag at once, without asking", async () => {
      renderTags();
      await userEvent.click(screen.getByRole("button", { name: "Delete tag admin" }));
      expect(mocked.tagsDelete).toHaveBeenCalledWith("tag-admin");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("asks before deleting a tag in use, and names the recordings it affects", async () => {
      renderTags();
      await userEvent.click(screen.getByRole("button", { name: "Delete tag research" }));
      const dialog = await screen.findByRole("dialog", { name: "Delete the tag research?" });
      expect(within(dialog).getByText(/3 recordings carry it/)).toBeInTheDocument();
      expect(mocked.tagsDelete).not.toHaveBeenCalled();
    });

    it("deletes a tag in use once confirmed", async () => {
      renderTags();
      await userEvent.click(screen.getByRole("button", { name: "Delete tag research" }));
      await userEvent.click(await screen.findByRole("button", { name: "Delete" }));
      expect(mocked.tagsDelete).toHaveBeenCalledWith("tag-research");
    });

    it("keeps a tag in use when the confirmation is cancelled", async () => {
      renderTags();
      await userEvent.click(screen.getByRole("button", { name: "Delete tag research" }));
      await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
      expect(mocked.tagsDelete).not.toHaveBeenCalled();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("toasts when the delete fails", async () => {
      mocked.tagsDelete.mockRejectedValueOnce({ kind: "db", message: "Locked", code: null });
      renderTags();
      await userEvent.click(screen.getByRole("button", { name: "Delete tag admin" }));
      expect(await screen.findByText("Could not delete the tag: Locked")).toBeInTheDocument();
    });
  });

  describe("adding", () => {
    it("creates the typed tag on Enter and clears the field", async () => {
      renderTags();
      await userEvent.type(screen.getByLabelText("New tag"), "billing{Enter}");
      expect(mocked.tagsSave).toHaveBeenCalledWith(null, "billing");
      expect(screen.getByLabelText("New tag")).toHaveValue("");
    });

    it("trims the name", async () => {
      renderTags();
      await userEvent.type(screen.getByLabelText("New tag"), "  billing  {Enter}");
      expect(mocked.tagsSave).toHaveBeenCalledWith(null, "billing");
    });

    it("ignores Enter on a blank field", async () => {
      renderTags();
      await userEvent.type(screen.getByLabelText("New tag"), "   {Enter}");
      expect(mocked.tagsSave).not.toHaveBeenCalled();
    });

    it("keeps the typed text and toasts when the save fails", async () => {
      mocked.tagsSave.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      renderTags();
      await userEvent.type(screen.getByLabelText("New tag"), "billing{Enter}");
      expect(await screen.findByText("Could not save the tag: Disk full")).toBeInTheDocument();
      expect(screen.getByLabelText("New tag")).toHaveValue("billing");
    });
  });
});
