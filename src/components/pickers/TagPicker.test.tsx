import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore } from "../../state/store";
import { projectsStore } from "../../test/projectFixtures";
import { Toasts } from "../Toasts";
import { TagPicker } from "./TagPicker";

const mocked = vi.mocked(api, true);

/** Holds the chosen ids like a form would, and reports every change. */
function Harness({ initial, onChange }: { initial: string[]; onChange: (ids: string[]) => void }) {
  const [value, setValue] = useState(initial);
  return (
    <TagPicker
      value={value}
      onChange={(ids) => {
        setValue(ids);
        onChange(ids);
      }}
    />
  );
}

function renderPicker(initial: string[] = []) {
  const onChange = vi.fn();
  resetStore(projectsStore());
  render(
    <>
      <Harness initial={initial} onChange={onChange} />
      <Toasts />
    </>,
  );
  return { onChange };
}

const field = () => screen.getByRole("combobox", { name: "Tags" });
const optionTexts = () => within(screen.getByRole("listbox", { name: "Tags" })).getAllByRole("option").map((o) => o.textContent);

describe("TagPicker", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows the chosen tags as a · b", () => {
    renderPicker(["tag-admin", "tag-research"]);
    expect(screen.getByText("admin · research")).toBeInTheDocument();
  });

  it("lists the tags that are not chosen yet", async () => {
    renderPicker(["tag-admin"]);
    await userEvent.click(field());
    expect(optionTexts()).toEqual(["research", "development", "maintenance", "internal meeting", "external meeting", "support"]);
  });

  it("filters the list by the typed text", async () => {
    renderPicker();
    await userEvent.type(field(), "meeting");
    expect(optionTexts()).toEqual(["internal meeting", "external meeting", 'New tag "meeting"']);
  });

  describe("adding", () => {
    it("adds the highlighted tag on Enter and clears the query", async () => {
      const { onChange } = renderPicker(["tag-admin"]);
      await userEvent.type(field(), "res{Enter}");
      expect(onChange).toHaveBeenCalledWith(["tag-admin", "tag-research"]);
      expect(screen.getByText("admin · research")).toBeInTheDocument();
      expect(field()).toHaveValue("");
    });

    it("adds the tag moved to with the arrow keys", async () => {
      const { onChange } = renderPicker();
      await userEvent.type(field(), "{ArrowDown}{Enter}");
      expect(onChange).toHaveBeenCalledWith(["tag-research"]);
    });

    it("adds a clicked tag", async () => {
      const { onChange } = renderPicker();
      await userEvent.click(field());
      await userEvent.click(screen.getByRole("option", { name: "support" }));
      expect(onChange).toHaveBeenCalledWith(["tag-support"]);
    });
  });

  describe("creating", () => {
    it("offers New tag for an unknown name", async () => {
      renderPicker();
      await userEvent.type(field(), "billing");
      expect(optionTexts()).toEqual(['New tag "billing"']);
    });

    it("offers no New tag for an existing name, whatever its case", async () => {
      renderPicker();
      await userEvent.type(field(), "ADMIN");
      expect(optionTexts()).toEqual(["admin"]);
    });

    it("creates the tag on Enter and adds the created one", async () => {
      const { onChange } = renderPicker(["tag-admin"]);
      await userEvent.type(field(), "billing{Enter}");
      expect(mocked.tagsSave).toHaveBeenCalledWith(null, "billing");
      await vi.waitFor(() => expect(onChange).toHaveBeenCalledWith(["tag-admin", "tag-billing"]));
    });

    it("adds nothing and toasts when the tag cannot be created", async () => {
      mocked.tagsSave.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      const { onChange } = renderPicker();
      await userEvent.type(field(), "billing{Enter}");
      expect(await screen.findByText("Could not save the tag: Disk full")).toBeInTheDocument();
      expect(onChange).not.toHaveBeenCalled();
    });
  });

  describe("Backspace", () => {
    it("removes the last tag when the query is empty", async () => {
      const { onChange } = renderPicker(["tag-admin", "tag-research"]);
      await userEvent.click(field());
      await userEvent.keyboard("{Escape}{Backspace}");
      expect(onChange).toHaveBeenCalledWith(["tag-admin"]);
      expect(screen.getByText("admin")).toBeInTheDocument();
    });

    it("only edits the query while there is one", async () => {
      const { onChange } = renderPicker(["tag-admin"]);
      await userEvent.type(field(), "ab{Backspace}");
      expect(field()).toHaveValue("a");
      expect(onChange).not.toHaveBeenCalled();
    });

    it("does nothing without tags", async () => {
      const { onChange } = renderPicker();
      await userEvent.click(field());
      await userEvent.keyboard("{Backspace}");
      expect(onChange).not.toHaveBeenCalled();
    });
  });

  it("closes the list with Escape", async () => {
    renderPicker();
    await userEvent.click(field());
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });
});
