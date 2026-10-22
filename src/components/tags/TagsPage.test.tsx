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
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.tagsSave.mockReset();
  });

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

  describe("renaming", () => {
    const admin = tag({ name: "admin" });
    const support = tag({ name: "support", recordings: 1 });
    const analysis = tag({ id: "tag-research", name: "analysis", recordings: 3 });
    const rowAt = (i: number) => screen.getAllByRole("listitem")[i];
    const researchRow = () => rowAt(1);
    const field = () => screen.getByRole<HTMLInputElement>("textbox", { name: "Tag name" });
    const noField = () => screen.queryByRole("textbox", { name: "Tag name" });
    const reloadsRenamed = () => mocked.tagsList.mockResolvedValueOnce([admin, analysis, support]);
    const startRename = () => userEvent.click(screen.getByRole("button", { name: "Rename tag research" }));
    const duplicate = { kind: "invalid_input", message: "A tag with this name already exists", code: null, field: "name" };

    describe("opening the field", () => {
      it("opens focused with the whole name selected from the Rename button", async () => {
        renderTags();
        await startRename();
        expect(field()).toHaveValue("research");
        expect(field()).toHaveFocus();
        expect([field().selectionStart, field().selectionEnd]).toEqual([0, "research".length]);
      });

      it("opens on a double click on the name", async () => {
        renderTags();
        await userEvent.dblClick(screen.getByText("research"));
        expect(field()).toHaveValue("research");
        expect(field()).toHaveFocus();
      });

      it("opens on Enter once the row is reached with Tab", async () => {
        renderTags();
        await userEvent.tab();
        await userEvent.tab();
        await userEvent.tab();
        await userEvent.tab();
        expect(researchRow()).toHaveFocus();
        await userEvent.keyboard("{Enter}");
        expect(field()).toHaveValue("research");
        expect(field()).toHaveFocus();
      });
    });

    describe("saving", () => {
      it("saves the typed name on Enter, closes the field and returns focus to the row", async () => {
        reloadsRenamed();
        renderTags();
        await startRename();
        await userEvent.keyboard("analysis{Enter}");
        await vi.waitFor(() => expect(noField()).not.toBeInTheDocument());
        expect(mocked.tagsSave).toHaveBeenCalledWith("tag-research", "analysis");
        expect(researchRow()).toHaveTextContent("analysis");
        expect(researchRow()).toHaveFocus();
      });

      it("saves when the field loses focus after an edit", async () => {
        reloadsRenamed();
        renderTags();
        await startRename();
        await userEvent.keyboard("analysis");
        await userEvent.tab();
        expect(await screen.findByText("analysis")).toBeInTheDocument();
        expect(mocked.tagsSave).toHaveBeenCalledWith("tag-research", "analysis");
        expect(noField()).not.toBeInTheDocument();
      });

      it("trims surrounding spaces", async () => {
        reloadsRenamed();
        renderTags();
        await startRename();
        await userEvent.keyboard("  analysis  {Enter}");
        await vi.waitFor(() => expect(noField()).not.toBeInTheDocument());
        expect(mocked.tagsSave).toHaveBeenCalledWith("tag-research", "analysis");
      });

      it("renames once when Enter is followed by the field losing focus", async () => {
        let release!: () => void;
        const gate = new Promise<void>((resolve) => (release = resolve));
        let saved = false;
        mocked.tagsSave.mockImplementation((id, name) => {
          if (saved && id === "tag-research") return Promise.reject(Object.assign(new Error("Saved twice"), { kind: "io", code: null }));
          saved = true;
          return gate.then(() => ({ id: id ?? "", name, createdAt: 0 }));
        });
        reloadsRenamed();
        renderTags();
        await startRename();
        await userEvent.keyboard("analysis{Enter}");
        await userEvent.tab();
        release();
        await vi.waitFor(() => expect(researchRow()).toHaveTextContent("analysis"));
        expect(noField()).not.toBeInTheDocument();
        expect(screen.queryByText(/Could not save the tag/)).not.toBeInTheDocument();
      });
    });

    describe("leaving without saving", () => {
      it("cancels on Escape, keeping the name and the focus on the row", async () => {
        renderTags();
        await startRename();
        await userEvent.keyboard("analysis{Escape}");
        expect(noField()).not.toBeInTheDocument();
        expect(mocked.tagsSave).not.toHaveBeenCalled();
        expect(researchRow()).toHaveTextContent("research");
        expect(researchRow()).toHaveFocus();
      });

      it.each([
        ["emptied", ""],
        ["whitespace-only", "   "],
        ["unchanged", "research"],
      ])("closes without saving when the name is %s", async (_, text) => {
        renderTags();
        await startRename();
        await userEvent.clear(field());
        if (text) await userEvent.keyboard(text);
        await userEvent.keyboard("{Enter}");
        expect(noField()).not.toBeInTheDocument();
        expect(mocked.tagsSave).not.toHaveBeenCalled();
        expect(researchRow()).toHaveTextContent("research");
      });
    });

    describe("when the save fails", () => {
      it("keeps the field open and shows a taken name inline, without a toast", async () => {
        mocked.tagsSave.mockRejectedValueOnce(duplicate);
        renderTags();
        await startRename();
        await userEvent.keyboard("admin{Enter}");
        expect(await screen.findByRole("alert")).toHaveTextContent("A tag with this name already exists");
        expect(field()).toHaveAttribute("aria-invalid", "true");
        expect(screen.queryByText(/Could not save the tag/)).not.toBeInTheDocument();
      });

      it("clears the inline error when typing again", async () => {
        mocked.tagsSave.mockRejectedValueOnce(duplicate);
        renderTags();
        await startRename();
        await userEvent.keyboard("admin{Enter}");
        await screen.findByRole("alert");
        await userEvent.type(field(), "2");
        expect(screen.queryByRole("alert")).not.toBeInTheDocument();
        expect(field()).toHaveAttribute("aria-invalid", "false");
      });

      it("keeps the field open and toasts any other failure, without an inline error", async () => {
        mocked.tagsSave.mockRejectedValueOnce({ kind: "io", message: "disk", code: null });
        renderTags();
        await startRename();
        await userEvent.keyboard("analysis{Enter}");
        expect(await screen.findByText("Could not save the tag: disk")).toBeInTheDocument();
        expect(field()).toHaveValue("analysis");
        expect(field()).toHaveAttribute("aria-invalid", "false");
        expect(within(researchRow()).queryByRole("alert")).not.toBeInTheDocument();
      });
    });
  });

  describe("adding", () => {
    it("creates the typed tag on Enter and clears the field", async () => {
      renderTags();
      await userEvent.type(screen.getByLabelText("New tag"), "billing{Enter}");
      expect(mocked.tagsSave).toHaveBeenCalledWith(null, "billing");
      await vi.waitFor(() => expect(screen.getByLabelText("New tag")).toHaveValue(""));
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
