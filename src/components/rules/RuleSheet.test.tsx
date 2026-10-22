import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type RuleInput } from "../../api";
import { resetStore } from "../../state/store";
import { at } from "../../test/dayFixtures";
import { projectItem, projectsStore } from "../../test/projectFixtures";
import { previewHit, rule, rulePreview } from "../../test/ruleFixtures";
import { Dialogs } from "../Dialogs";
import { Toasts } from "../Toasts";
import { RuleSheet } from "./RuleSheet";

const mocked = vi.mocked(api, true);

const website = projectItem({ project: { id: "project-1", name: "Website" } });
const slack = rule({ id: "r-slack", name: "Slack", note: "chat", conditions: [{ id: "c1", field: "app", op: "is", value: "Slack" }] });
const fieldError = (path: string, message: string) => ({ kind: "invalid_input", message: "Invalid", code: null, field: { path, message } });
const save = () => userEvent.click(screen.getByRole("button", { name: "Save" }));

const draft: RuleInput = {
  name: "Safari → Website",
  projectId: "project-1",
  enabled: true,
  note: "",
  tagId: null,
  conditions: [
    { field: "app", op: "is", value: "Safari" },
    { field: "title", op: "contains", value: "docs" },
  ],
};

function renderSheet(props: Partial<ComponentProps<typeof RuleSheet>> = {}) {
  const onClose = vi.fn();
  resetStore({ ...projectsStore({ projects: [website] }), rules: [slack] });
  render(
    <>
      <RuleSheet onClose={onClose} {...props} />
      <Dialogs />
      <Toasts />
    </>,
  );
  return { onClose };
}

const preview = () => within(screen.getByRole("region", { name: "Matching activity" }));

describe("RuleSheet", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.rulePreview.mockResolvedValue(rulePreview());
  });

  describe("creating", () => {
    it("saves the typed name, the picked project and the condition, then closes", async () => {
      const { onClose } = renderSheet();
      expect(screen.getByRole("heading", { name: "New rule" })).toBeInTheDocument();
      await userEvent.type(screen.getByLabelText("Name"), "Chat");
      await userEvent.click(screen.getByRole("combobox", { name: "Project" }));
      await userEvent.keyboard("{ArrowDown}");
      await userEvent.click(await screen.findByRole("option", { name: "Website" }));
      await userEvent.type(screen.getByLabelText("Condition 1 value"), "Slack");
      await save();
      expect(mocked.ruleSave).toHaveBeenCalledWith(null, { name: "Chat", projectId: "project-1", enabled: true, note: "", tagId: null, conditions: [{ field: "app", op: "is", value: "Slack" }] });
      expect(onClose).toHaveBeenCalled();
    });

    it("saves the picked tag", async () => {
      renderSheet({ draft });
      await userEvent.click(screen.getByRole("combobox", { name: "Tag" }));
      await userEvent.keyboard("{ArrowDown}");
      await userEvent.click(screen.getByRole("option", { name: "support" }));
      await save();
      expect(mocked.ruleSave).toHaveBeenCalledWith(null, expect.objectContaining({ tagId: "tag-support" }));
    });

    it("adds a condition, with the field and comparison the user picks", async () => {
      renderSheet({ draft: { ...draft, conditions: [draft.conditions[0]] } });
      await userEvent.click(screen.getByRole("button", { name: "Add condition" }));
      await userEvent.selectOptions(screen.getByLabelText("Condition 2 field"), "Domain");
      await userEvent.selectOptions(screen.getByLabelText("Condition 2 comparison"), "begins with");
      await userEvent.type(screen.getByLabelText("Condition 2 value"), "example");
      await save();
      expect(mocked.ruleSave).toHaveBeenCalledWith(null, expect.objectContaining({ conditions: [draft.conditions[0], { field: "domain", op: "begins_with", value: "example" }] }));
    });

    it("removes a condition, but never the last one", async () => {
      renderSheet({ draft });
      await userEvent.click(screen.getByRole("button", { name: "Remove condition 1" }));
      expect(screen.queryByLabelText("Condition 2 value")).not.toBeInTheDocument();
      expect(screen.getByLabelText("Condition 1 value")).toHaveValue("docs");
      expect(screen.getByRole("button", { name: "Remove condition 1" })).toBeDisabled();
    });

    it("offers no Delete", () => {
      renderSheet();
      expect(screen.queryByRole("button", { name: "Delete" })).not.toBeInTheDocument();
    });

    it("starts from a draft", () => {
      renderSheet({ draft });
      expect(screen.getByLabelText("Name")).toHaveValue("Safari → Website");
      expect(screen.getByRole("combobox", { name: "Project" })).toHaveValue("Website");
      expect(screen.getByLabelText("Condition 1 value")).toHaveValue("Safari");
      expect(screen.getByLabelText("Condition 2 comparison")).toHaveValue("contains");
    });
  });

  describe("editing", () => {
    it("starts from the rule and saves its id with the changes", async () => {
      const { onClose } = renderSheet({ id: "r-slack" });
      expect(screen.getByRole("heading", { name: "Edit rule" })).toBeInTheDocument();
      expect(screen.getByLabelText("Name")).toHaveValue("Slack");
      expect(screen.getByLabelText("Note")).toHaveValue("chat");
      await userEvent.clear(screen.getByLabelText("Condition 1 value"));
      await userEvent.type(screen.getByLabelText("Condition 1 value"), "Teams");
      await userEvent.click(screen.getByRole("switch", { name: "Suggest recordings" }));
      await save();
      expect(mocked.ruleSave).toHaveBeenCalledWith("r-slack", { name: "Slack", projectId: "project-1", enabled: false, note: "chat", tagId: null, conditions: [{ field: "app", op: "is", value: "Teams" }] });
      expect(onClose).toHaveBeenCalled();
    });

    it("shows the tag of the rule", () => {
      resetStore({ ...projectsStore({ projects: [website] }), rules: [rule({ ...slack, tagId: "tag-research" })] });
      render(<RuleSheet id="r-slack" onClose={vi.fn()} />);
      expect(screen.getByRole("combobox", { name: "Tag" })).toHaveValue("research");
    });

    it("closes without saving on Cancel", async () => {
      const { onClose } = renderSheet({ id: "r-slack" });
      await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
      expect(onClose).toHaveBeenCalled();
      expect(mocked.ruleSave).not.toHaveBeenCalled();
    });

    it("deletes the rule after confirming", async () => {
      const { onClose } = renderSheet({ id: "r-slack" });
      await userEvent.click(screen.getByRole("button", { name: "Delete" }));
      const confirm = within(await screen.findByRole("dialog", { name: "Delete rule" }));
      expect(mocked.ruleDelete).not.toHaveBeenCalled();
      await userEvent.click(confirm.getByRole("button", { name: "Delete" }));
      await vi.waitFor(() => expect(onClose).toHaveBeenCalled());
      expect(mocked.ruleDelete).toHaveBeenCalledWith("r-slack");
    });

    it("keeps the rule when the delete is cancelled", async () => {
      const { onClose } = renderSheet({ id: "r-slack" });
      await userEvent.click(screen.getByRole("button", { name: "Delete" }));
      await userEvent.click(within(await screen.findByRole("dialog", { name: "Delete rule" })).getByRole("button", { name: "Cancel" }));
      expect(mocked.ruleDelete).not.toHaveBeenCalled();
      expect(onClose).not.toHaveBeenCalled();
    });
  });

  describe("errors", () => {
    it("marks the condition row a field error names and keeps the sheet open", async () => {
      mocked.ruleSave.mockRejectedValueOnce(fieldError("conditions.1.value", "Needs a value"));
      const { onClose } = renderSheet({ draft });
      await save();
      const row = await screen.findByRole("group", { name: "Condition 2" });
      await within(row).findByRole("alert");
      expect(row).toHaveAttribute("aria-invalid", "true");
      expect(within(row).getByRole("alert")).toHaveTextContent("Needs a value");
      expect(screen.getByRole("group", { name: "Condition 1" })).not.toHaveAttribute("aria-invalid");
      expect(onClose).not.toHaveBeenCalled();
    });

    it("shows the error of the name below its field", async () => {
      mocked.ruleSave.mockRejectedValueOnce(fieldError("name", "Cannot be empty"));
      renderSheet({ draft });
      await save();
      expect(await screen.findByRole("alert")).toHaveTextContent("Cannot be empty");
    });

    it("clears the marks on the next save", async () => {
      mocked.ruleSave.mockRejectedValueOnce(fieldError("conditions.1.value", "Needs a value"));
      renderSheet({ draft });
      await save();
      await screen.findByText("Needs a value");
      await save();
      await vi.waitFor(() => expect(screen.queryByText("Needs a value")).not.toBeInTheDocument());
    });

    it("toasts an error without a field and keeps the sheet open", async () => {
      mocked.ruleSave.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      const { onClose } = renderSheet({ draft });
      await save();
      expect(await screen.findByRole("alert")).toHaveTextContent("Could not save the rule: Database locked");
      expect(onClose).not.toHaveBeenCalled();
    });
  });

  describe("preview", () => {
    it("lists the matching activity of the last 7 days with the total", async () => {
      mocked.rulePreview.mockResolvedValue(
        rulePreview({
          hits: [previewHit("2026-03-10", { id: "h1", app: "Safari", title: "Docs home", start: at("09:15"), end: at("09:45") }, "Tue, Mar 10"), previewHit("2026-03-09", { id: "h2", app: "Safari", title: "Docs API", start: at("14:00", "2026-03-09"), end: at("15:00", "2026-03-09") }, "Mon, Mar 9")],
          totalMs: 90 * 60_000,
          days: 2,
        }),
      );
      renderSheet({ draft });
      expect(await preview().findByText("1h 30m on 2 days")).toBeInTheDocument();
      expect(preview().getByText("Docs home")).toBeInTheDocument();
      expect(preview().getByText("Docs API")).toBeInTheDocument();
      expect(preview().getByText("Tue, Mar 10 09:15")).toBeInTheDocument();
      expect(preview().getByText("Mon, Mar 9 14:00")).toBeInTheDocument();
      expect(mocked.rulePreview).toHaveBeenLastCalledWith(expect.objectContaining({ conditions: draft.conditions }), 7);
    });

    it("says so when nothing matches", async () => {
      renderSheet({ draft });
      expect(await preview().findByText("Nothing matches")).toBeInTheDocument();
    });

    it("says 1 day for a single day", async () => {
      mocked.rulePreview.mockResolvedValue(rulePreview({ hits: [previewHit("2026-03-10", { id: "h1" })], totalMs: 30 * 60_000, days: 1 }));
      renderSheet({ draft });
      expect(await preview().findByText("30m on 1 day")).toBeInTheDocument();
    });

    it("asks again with the typed value after the typing paused", async () => {
      renderSheet({ draft });
      await preview().findByText("Nothing matches");
      mocked.rulePreview.mockResolvedValue(rulePreview({ hits: [previewHit("2026-03-10", { id: "h1", title: "Docs home" })], totalMs: 30 * 60_000, days: 1 }));
      await userEvent.type(screen.getByLabelText("Condition 2 value"), "x");
      expect(await preview().findByText("Docs home")).toBeInTheDocument();
      expect(mocked.rulePreview).toHaveBeenLastCalledWith(expect.objectContaining({ conditions: [draft.conditions[0], { field: "title", op: "contains", value: "docsx" }] }), 7);
    });

    it("waits for a value in every condition that needs one", async () => {
      renderSheet({ draft: { ...draft, conditions: [{ field: "title", op: "contains", value: "" }] } });
      expect(preview().getByText("Give each condition a value to see what it matches.")).toBeInTheDocument();
      await userEvent.type(screen.getByLabelText("Condition 1 value"), "docs");
      expect(await preview().findByText("Nothing matches")).toBeInTheDocument();
      expect(preview().queryByText("Give each condition a value to see what it matches.")).not.toBeInTheDocument();
    });

    it("previews a condition that is empty with is", async () => {
      renderSheet({ draft: { ...draft, conditions: [{ field: "path", op: "is", value: "" }] } });
      expect(await preview().findByText("Nothing matches")).toBeInTheDocument();
    });

    it("shows the error when the preview fails", async () => {
      mocked.rulePreview.mockRejectedValue({ kind: "invalid_input", message: "Bad pattern", code: null });
      renderSheet({ draft });
      expect(await preview().findByText("Bad pattern")).toBeInTheDocument();
    });
  });
});
