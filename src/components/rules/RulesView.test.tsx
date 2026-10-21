import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore } from "../../state/store";
import { projectItem, projectsStore } from "../../test/projectFixtures";
import { rule } from "../../test/ruleFixtures";
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
  beforeEach(() => vi.clearAllMocks());

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
