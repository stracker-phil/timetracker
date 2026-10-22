import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { handleKeydown, installCommands } from "../../commands";
import { resetStore, useStore } from "../../state/store";
import { projectItem, projectsStore } from "../../test/projectFixtures";
import { rule, ruleDay } from "../../test/ruleFixtures";
import { Dialogs } from "../Dialogs";
import { RulesView } from "./RulesView";

const mocked = vi.mocked(api, true);

const ROW_PX = 40;
const slack = rule({ id: "r-slack", name: "Slack", position: 0, conditions: [{ id: "c1", field: "app", op: "is", value: "Slack" }] });
const standup = rule({
  id: "r-standup",
  name: "Standup",
  position: 1,
  conditions: [
    { id: "c2", field: "app", op: "is", value: "Zoom" },
    { id: "c3", field: "title", op: "contains", value: "standup" },
  ],
});
const docs = rule({ id: "r-docs", name: "Docs", position: 2, enabled: false, conditions: [{ id: "c4", field: "domain", op: "is", value: "example.com" }] });
const website = projectItem({ project: { id: "project-1", name: "Website" } });

async function renderView(rules = [slack, standup, docs]) {
  mocked.rulesList.mockResolvedValue(rules);
  resetStore({ ...projectsStore({ projects: [website] }), rules });
  render(
    <>
      <RulesView />
      <Dialogs />
    </>,
  );
  await screen.findByRole("list", { name: "Rules in order" });
}

const names = () => within(screen.getByRole("list", { name: "Rules in order" })).getAllByRole("switch").map((s) => s.getAttribute("aria-label")?.replace("Suggest from ", ""));

/** Presses the grip of `name`'s row and releases it `dy` px lower. */
function dragGrip(name: string, dy: number) {
  const grip = screen.getByLabelText(`Drag to reorder ${name}`);
  vi.spyOn(grip.closest("li") as HTMLElement, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 400, ROW_PX));
  fireEvent.pointerDown(grip, { button: 0, clientY: 100 });
  fireEvent.pointerMove(grip, { clientY: 100 + dy });
  fireEvent.pointerUp(grip, { clientY: 100 + dy });
}

describe("RulesView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.rulesToday.mockResolvedValue([]);
  });

  describe("listing", () => {
    it("lists the rules in order with their project and conditions", async () => {
      await renderView();
      expect(names()).toEqual(["Slack", "Standup", "Docs"]);
      const row = screen.getByText("Standup").closest("li") as HTMLElement;
      expect(within(row).getByText("Website")).toBeInTheDocument();
      expect(within(row).getByText('app is "Zoom" · title contains "standup"')).toBeInTheDocument();
    });

    it("shows the enabled state of each rule in its switch", async () => {
      await renderView();
      expect(screen.getByRole("switch", { name: "Suggest from Slack" })).toBeChecked();
      expect(screen.getByRole("switch", { name: "Suggest from Docs" })).not.toBeChecked();
    });

    it("says so when there are no rules", async () => {
      mocked.rulesList.mockResolvedValue([]);
      resetStore(projectsStore());
      render(<RulesView />);
      expect(await screen.findByText("No rules yet.")).toBeInTheDocument();
    });

    it("loads the rules when it opens", async () => {
      mocked.rulesList.mockResolvedValue([slack]);
      resetStore(projectsStore({ projects: [website] }));
      render(<RulesView />);
      expect(await screen.findByText("Slack")).toBeInTheDocument();
    });

    it("explains that suggestions wait in the Day view until they are approved or dismissed", async () => {
      await renderView();
      expect(screen.getByText(/the first matching rule wins\. Suggestions wait in the Day view until you approve or dismiss them/)).toBeInTheDocument();
    });
  });

  describe("today", () => {
    const rowOf = (name: string) => within(screen.getByText(name).closest("li") as HTMLElement);

    it("tells the matched time and the suggestions to review of an enabled rule", async () => {
      mocked.rulesToday.mockResolvedValue([ruleDay({ ruleId: "r-slack", matchedMs: 80 * 60_000, pending: 2, pendingMs: 30 * 60_000 })]);
      await renderView();
      expect(await rowOf("Slack").findByText("1h 20m today · 2 to review")).toBeInTheDocument();
    });

    it("leaves out the review count when nothing waits", async () => {
      mocked.rulesToday.mockResolvedValue([ruleDay({ ruleId: "r-slack", matchedMs: 45 * 60_000 })]);
      await renderView();
      expect(await rowOf("Slack").findByText("45m today")).toBeInTheDocument();
    });

    it("says Nothing today for an enabled rule without matches", async () => {
      mocked.rulesToday.mockResolvedValue([ruleDay({ ruleId: "r-slack", matchedMs: 45 * 60_000 })]);
      await renderView();
      await rowOf("Slack").findByText("45m today");
      expect(rowOf("Standup").getByText("Nothing today")).toBeInTheDocument();
    });

    it("says Off for a disabled rule, whatever it matched", async () => {
      mocked.rulesToday.mockResolvedValue([ruleDay({ ruleId: "r-docs", matchedMs: 45 * 60_000, pending: 1 })]);
      await renderView();
      expect(rowOf("Docs").getByText("Off")).toBeInTheDocument();
      expect(rowOf("Docs").queryByText(/today/)).not.toBeInTheDocument();
    });

    it("still lists the rules when the day cannot be read", async () => {
      mocked.rulesToday.mockRejectedValue({ kind: "db", message: "Locked", code: null });
      await renderView();
      expect(rowOf("Slack").getByText("Nothing today")).toBeInTheDocument();
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    });
  });

  describe("enabling", () => {
    it("turns a rule off with its switch", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("switch", { name: "Suggest from Slack" }));
      expect(mocked.ruleSetEnabled).toHaveBeenCalledWith("r-slack", false);
      await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Suggest from Slack" })).not.toBeChecked());
    });

    it("turns a rule on with its switch", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("switch", { name: "Suggest from Docs" }));
      expect(mocked.ruleSetEnabled).toHaveBeenCalledWith("r-docs", true);
    });
  });

  describe("reordering", () => {
    it.each([
      ["a row down by one row height moves it one place down", "Slack", ROW_PX, ["r-standup", "r-slack", "r-docs"]],
      ["a row up by one row height moves it one place up", "Docs", -ROW_PX, ["r-slack", "r-docs", "r-standup"]],
      ["a row down by two rows moves it to the end", "Slack", 2 * ROW_PX, ["r-standup", "r-docs", "r-slack"]],
      ["a row dragged past the end stays last", "Standup", 10 * ROW_PX, ["r-slack", "r-docs", "r-standup"]],
    ])("dragging %s", async (_name, name, dy, expected) => {
      await renderView();
      dragGrip(name, dy);
      expect(mocked.rulesReorder).toHaveBeenCalledWith(expected);
    });

    it("shows the new order at once", async () => {
      await renderView();
      mocked.rulesReorder.mockReturnValueOnce(new Promise(() => {}));
      dragGrip("Slack", ROW_PX);
      await vi.waitFor(() => expect(names()).toEqual(["Standup", "Slack", "Docs"]));
    });

    it("keeps the order when the write fails", async () => {
      await renderView();
      mocked.rulesReorder.mockRejectedValueOnce({ kind: "invalid_input", message: "Unknown rule", code: null });
      dragGrip("Slack", ROW_PX);
      await vi.waitFor(() => expect(mocked.rulesReorder).toHaveBeenCalled());
      await vi.waitFor(() => expect(names()).toEqual(["Slack", "Standup", "Docs"]));
    });

    it.each([
      ["a drag shorter than half a row", ROW_PX / 2 - 1],
      ["a press released where it was pressed", 0],
    ])("does nothing for %s", async (_name, dy) => {
      await renderView();
      dragGrip("Standup", dy);
      expect(mocked.rulesReorder).not.toHaveBeenCalled();
    });
  });

  describe("reordering with ⌥↑ and ⌥↓", () => {
    const rows = [slack, standup, docs];
    const rowButton = (name: string) => within(screen.getByText(name).closest("li") as HTMLElement).getByRole("button");

    async function renderRules() {
      mocked.rulesReorder.mockImplementation(async (ids) => ids.map((id, position) => ({ ...rows.find((r) => r.id === id)!, position })));
      await renderView();
      useStore.setState({ view: "rules" });
      installCommands();
    }

    beforeEach(() => window.addEventListener("keydown", handleKeydown, true));

    afterEach(() => {
      window.removeEventListener("keydown", handleKeydown, true);
      mocked.rulesReorder.mockResolvedValue([]);
    });

    it.each([
      ["⌥↓ moves the focused rule one place down", "Slack", "{Alt>}{ArrowDown}{/Alt}", ["r-standup", "r-slack", "r-docs"]],
      ["⌥↑ moves the focused rule one place up", "Docs", "{Alt>}{ArrowUp}{/Alt}", ["r-slack", "r-docs", "r-standup"]],
    ])("%s", async (_name, name, keys, expected) => {
      await renderRules();
      rowButton(name).focus();
      await userEvent.keyboard(keys);
      expect(mocked.rulesReorder).toHaveBeenCalledWith(expected);
    });

    it("keeps the focus in the moved rule's row", async () => {
      await renderRules();
      rowButton("Slack").focus();
      await userEvent.keyboard("{Alt>}{ArrowDown}{/Alt}");
      await vi.waitFor(() => expect(names()).toEqual(["Standup", "Slack", "Docs"]));
      expect(rowButton("Slack")).toHaveFocus();
    });

    it.each([
      ["⌥↑ on the first rule", "Slack", "{Alt>}{ArrowUp}{/Alt}"],
      ["⌥↓ on the last rule", "Docs", "{Alt>}{ArrowDown}{/Alt}"],
    ])("%s changes nothing", async (_name, name, keys) => {
      await renderRules();
      rowButton(name).focus();
      await userEvent.keyboard(keys);
      expect(mocked.rulesReorder).not.toHaveBeenCalled();
      expect(names()).toEqual(["Slack", "Standup", "Docs"]);
    });

    it("does nothing while the focus is outside the rows", async () => {
      await renderRules();
      await userEvent.keyboard("{Alt>}{ArrowDown}{/Alt}");
      expect(mocked.rulesReorder).not.toHaveBeenCalled();
    });
  });

  describe("dialogs", () => {
    it("opens an empty rule sheet with New rule", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: "New rule" }));
      const dialog = within(await screen.findByRole("dialog", { name: "New rule" }));
      expect(dialog.getByLabelText("Name")).toHaveValue("");
    });

    it("opens the sheet of a rule when its row is clicked", async () => {
      await renderView();
      await userEvent.click(within(screen.getByText("Standup").closest("li") as HTMLElement).getByRole("button"));
      const dialog = within(await screen.findByRole("dialog", { name: "Edit rule" }));
      expect(dialog.getByLabelText("Name")).toHaveValue("Standup");
    });
  });
});
