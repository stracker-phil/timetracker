import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore } from "../../state/store";
import { DEFAULT_TAGS, projectsStore, tag } from "../../test/projectFixtures";
import { Toasts } from "../Toasts";
import { TagPicker } from "./TagPicker";

const mocked = vi.mocked(api, true);

/** Holds the chosen id like a form would, and reports every change. */
function Harness({ initial, onChange }: { initial: string | null; onChange: (id: string | null) => void }) {
  const [value, setValue] = useState(initial);
  return (
    <TagPicker
      value={value}
      onChange={(id) => {
        setValue(id);
        onChange(id);
      }}
    />
  );
}

function renderPicker(initial: string | null = null, tags = DEFAULT_TAGS) {
  const onChange = vi.fn();
  resetStore(projectsStore({ tags }));
  render(
    <>
      <Harness initial={initial} onChange={onChange} />
      <Toasts />
    </>,
  );
  return { onChange };
}

const field = () => screen.getByRole("combobox", { name: "Tag" });
const optionTexts = () => within(screen.getByRole("listbox", { name: "Tags" })).getAllByRole("option").map((o) => o.textContent);

describe("TagPicker", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows the name of the chosen tag", () => {
    renderPicker("tag-admin");
    expect(field()).toHaveValue("admin");
  });

  it("shows No tag as the placeholder when nothing is chosen", () => {
    renderPicker();
    expect(field()).toHaveValue("");
    expect(field()).toHaveAttribute("placeholder", "No tag");
  });

  describe("emoji", () => {
    const tags = [tag({ name: "admin" }), tag({ name: "research", emoji: "🔬" })];

    it("shows an option's emoji before the tag name", async () => {
      renderPicker(null, tags);
      await userEvent.click(field());
      expect(optionTexts()).toEqual(["No tag", "admin", "🔬research"]);
    });

    it("shows the emoji of the chosen tag in the field", () => {
      renderPicker("tag-research", tags);
      expect(field().parentElement).toHaveTextContent("🔬");
    });

    it("shows no emoji for a chosen tag without one", () => {
      renderPicker("tag-admin", tags);
      expect(field().parentElement).not.toHaveTextContent("🔬");
    });
  });

  describe("opening", () => {
    it.each([null, "tag-admin"])("opens the list on Tab focus (value %j)", async (value) => {
      renderPicker(value);
      await userEvent.tab();
      expect(field()).toHaveFocus();
      expect(field()).toHaveAttribute("aria-expanded", "true");
      expect(screen.getByRole("listbox", { name: "Tags" })).toBeInTheDocument();
    });

    it.each([null, "tag-admin"])("opens the list on the first click (value %j)", async (value) => {
      renderPicker(value);
      await userEvent.click(field());
      expect(screen.getByRole("listbox", { name: "Tags" })).toBeInTheDocument();
    });

    it("reopens a closed list with a click on the focused field", async () => {
      renderPicker("tag-admin");
      await userEvent.tab();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      await userEvent.click(field());
      expect(screen.getByRole("listbox", { name: "Tags" })).toBeInTheDocument();
    });

    it("focuses and opens through the chevron on an unfocused field", async () => {
      renderPicker();
      await userEvent.click(screen.getByRole("button", { name: "Show tags" }));
      expect(field()).toHaveFocus();
      expect(screen.getByRole("listbox", { name: "Tags" })).toBeInTheDocument();
    });

    it("toggles the list with the chevron on a focused field", async () => {
      renderPicker();
      await userEvent.tab();
      await userEvent.click(screen.getByRole("button", { name: "Show tags" }));
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Show tags" }));
      expect(screen.getByRole("listbox", { name: "Tags" })).toBeInTheDocument();
    });

    it("highlights the current tag when opened by focus", async () => {
      renderPicker("tag-research");
      await userEvent.tab();
      const current = screen.getByRole("option", { name: "research" });
      expect(current).toHaveAttribute("aria-selected", "true");
      expect(current).toHaveAttribute("aria-checked", "true");
    });

    it.each(["{ArrowDown}", "{ArrowUp}", " "])("reopens a closed list on %j with the current tag highlighted and checked", async (key) => {
      renderPicker("tag-research");
      await userEvent.tab();
      await userEvent.keyboard("{Escape}");
      await userEvent.keyboard(key);
      const current = screen.getByRole("option", { name: "research" });
      expect(current).toHaveAttribute("aria-selected", "true");
      expect(current).toHaveAttribute("aria-checked", "true");
    });

    it("counts No tag as current when there is no tag", async () => {
      renderPicker();
      await userEvent.tab();
      const none = screen.getByRole("option", { name: "No tag" });
      expect(none).toHaveAttribute("aria-selected", "true");
      expect(none).toHaveAttribute("aria-checked", "true");
    });

    it("filters by the typed character with the first match highlighted", async () => {
      renderPicker();
      await userEvent.tab();
      await userEvent.keyboard("s");
      expect(field()).toHaveValue("s");
      expect(screen.getAllByRole("option")[0]).toHaveAttribute("aria-selected", "true");
    });
  });

  describe("the list", () => {
    it("starts with No tag, followed by every tag", async () => {
      renderPicker("tag-admin");
      await userEvent.click(field());
      expect(optionTexts()).toEqual(["No tag", "admin", "research", "development", "maintenance", "internal meeting", "external meeting", "support"]);
    });

    it("filters by the typed text and drops No tag", async () => {
      renderPicker();
      await userEvent.type(field(), "meeting");
      expect(optionTexts()).toEqual(["internal meeting", "external meeting", 'New tag "meeting"']);
    });

    it("closes with Escape and keeps the value", async () => {
      renderPicker("tag-admin");
      await userEvent.click(field());
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      expect(field()).toHaveValue("admin");
    });
  });

  describe("picking", () => {
    it("picks the highlighted tag on Enter and shows it", async () => {
      const { onChange } = renderPicker();
      await userEvent.type(field(), "res{Enter}");
      expect(onChange).toHaveBeenCalledWith("tag-research");
      expect(field()).toHaveValue("research");
    });

    it("picks the tag moved to with the arrow keys", async () => {
      const { onChange } = renderPicker();
      await userEvent.type(field(), "{ArrowDown}{Enter}");
      expect(onChange).toHaveBeenCalledWith("tag-admin");
    });

    it("picks a clicked tag", async () => {
      const { onChange } = renderPicker();
      await userEvent.click(field());
      await userEvent.click(screen.getByRole("option", { name: "support" }));
      expect(onChange).toHaveBeenCalledWith("tag-support");
    });

    it("replaces the chosen tag instead of adding a second one", async () => {
      const { onChange } = renderPicker("tag-admin");
      await userEvent.type(field(), "res{Enter}");
      expect(onChange).toHaveBeenLastCalledWith("tag-research");
      expect(field()).toHaveValue("research");
    });

    it("removes the tag when No tag is picked", async () => {
      const { onChange } = renderPicker("tag-admin");
      await userEvent.click(field());
      await userEvent.click(screen.getByRole("option", { name: "No tag" }));
      expect(onChange).toHaveBeenCalledWith(null);
      expect(field()).toHaveValue("");
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

    it("creates the tag on Enter and picks the created one", async () => {
      const { onChange } = renderPicker("tag-admin");
      await userEvent.type(field(), "billing{Enter}");
      expect(mocked.tagsSave).toHaveBeenCalledWith(null, "billing");
      await vi.waitFor(() => expect(onChange).toHaveBeenCalledWith("tag-billing"));
    });

    it("picks nothing and toasts when the tag cannot be created", async () => {
      mocked.tagsSave.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      const { onChange } = renderPicker();
      await userEvent.type(field(), "billing{Enter}");
      expect(await screen.findByText("Could not save the tag: Disk full")).toBeInTheDocument();
      expect(onChange).not.toHaveBeenCalled();
    });
  });

  describe("Backspace", () => {
    it("clears the tag while the list is closed", async () => {
      const { onChange } = renderPicker("tag-admin");
      await userEvent.click(field());
      await userEvent.keyboard("{Escape}{Backspace}");
      expect(onChange).toHaveBeenCalledWith(null);
      expect(field()).toHaveValue("");
    });

    it("clears the tag while the list is open with an empty query", async () => {
      const { onChange } = renderPicker("tag-admin");
      await userEvent.click(field());
      await userEvent.keyboard("{Backspace}");
      expect(onChange).toHaveBeenCalledWith(null);
      expect(field()).toHaveValue("");
    });

    it("only edits the query while there is one", async () => {
      const { onChange } = renderPicker("tag-admin");
      await userEvent.type(field(), "ab{Backspace}");
      expect(field()).toHaveValue("a");
      expect(onChange).not.toHaveBeenCalled();
    });

    it("does nothing without a tag", async () => {
      const { onChange } = renderPicker();
      await userEvent.click(field());
      await userEvent.keyboard("{Backspace}");
      expect(onChange).not.toHaveBeenCalled();
    });
  });
});
