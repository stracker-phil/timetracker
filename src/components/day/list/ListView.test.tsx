import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE, type ListRow } from "../../../api";
import { handleKeydown, installCommands, runCommand } from "../../../commands";
import { flushUiState, resetStore, useStore } from "../../../state/store";
import { at, block, DAY, dayView, lane } from "../../../test/dayFixtures";
import { listRow, listView } from "../../../test/listFixtures";
import { projectItem, projectsStore } from "../../../test/projectFixtures";
import { settings } from "../../../test/settings";
import { Dialogs } from "../../Dialogs";
import { Toasts } from "../../Toasts";
import { DayPage } from "../DayPage";

const mocked = vi.mocked(api, true);

const MINUTE_MS = 60_000;

const website = projectItem({ project: { id: "p-site", name: "Website" } });

const docs = listRow({ app: "Safari", title: "Docs", url: "https://example.com/docs", firstStart: at("09:00"), lastEnd: at("10:00"), blockIds: ["a1"] });
const todo = listRow({ app: "Notes", title: "Todo", firstStart: at("10:00"), lastEnd: at("10:30"), coveredMs: 10 * MINUTE_MS, blockIds: ["a2"] });
const report = listRow({ app: "Preview", title: "Report.pdf", path: "/tmp/report.pdf", firstStart: at("11:00"), lastEnd: at("11:20"), blockIds: ["a3"] });
const inbox = listRow({
  app: "Mail",
  title: "Inbox",
  firstStart: at("13:00"),
  lastEnd: at("14:20"),
  totalMs: 50 * MINUTE_MS,
  spans: [
    [at("13:00"), at("13:20")],
    [at("13:40"), at("13:50")],
    [at("14:00"), at("14:20")],
  ],
  blockIds: ["a4", "a5", "a6"],
});

let over: Element | null;

function renderList(rows: ListRow[], patch: Parameters<typeof resetStore>[0] = {}) {
  mocked.listView.mockResolvedValue(listView({ rows }));
  mocked.dayView.mockResolvedValue(dayView({ lanes: [lane({ blocks: [block()] })] }));
  resetStore({
    day: DAY,
    permissionDialogHidden: true,
    settings: settings(),
    ...projectsStore({ projects: [website], recent: [website.project] }),
    uiState: { ...EMPTY_UI_STATE, dayMode: "list" },
    ...patch,
  });
  installCommands();
  return render(
    <>
      <DayPage />
      <Toasts />
      <Dialogs />
    </>,
  );
}

const activity = () => screen.findByRole("list", { name: "Activity" });
const rowEls = () => within(screen.getByRole("list", { name: "Activity" })).getAllByRole("listitem");
const rowOf = (title: string) => rowEls().find((r) => within(r).queryByRole("button", { name: title })) as HTMLElement;
const search = () => screen.getByRole("searchbox", { name: "Search activity" });
const chip = (name: string) => within(screen.getByRole("group", { name: "Projects" })).getByRole("button", { name });
const editorDialog = () => screen.findByRole("dialog", { name: "New recording" });

async function renderedRows(rows: ListRow[], patch: Parameters<typeof resetStore>[0] = {}) {
  renderList(rows, patch);
  await activity();
}

/** Presses `row`, moves to a target and releases there. */
function dragRow(row: HTMLElement, target: Element | null) {
  fireEvent.pointerDown(row, { button: 0, clientX: 10, clientY: 10 });
  fireEvent.pointerMove(row, { clientX: 60, clientY: 60 });
  over = target;
  fireEvent.pointerUp(row, { clientX: 60, clientY: 60 });
}

describe("ListView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    over = null;
    document.elementFromPoint = vi.fn(() => over);
    window.addEventListener("keydown", handleKeydown, true);
  });

  afterEach(() => {
    window.removeEventListener("keydown", handleKeydown, true);
    delete (document as { elementFromPoint?: unknown }).elementFromPoint;
    flushUiState();
    vi.useRealTimers();
  });

  describe("rows", () => {
    it("lists the rows in the order Rust gave them", async () => {
      await renderedRows([todo, docs, report]);
      expect(rowEls().map((r) => within(r).getByRole("button").textContent)).toEqual(["Todo", "Docs", "Report.pdf"]);
    });

    it("shows app, title, total time and where the activity was", async () => {
      await renderedRows([docs, report]);
      const row = within(rowOf("Docs"));
      expect(row.getByText("Safari")).toBeInTheDocument();
      expect(row.getByText("1:00")).toBeInTheDocument();
      expect(row.getByText("https://example.com/docs")).toBeInTheDocument();
      expect(within(rowOf("Report.pdf")).getByText("/tmp/report.pdf")).toBeInTheDocument();
    });

    it("hints at the number of spans only for a row with several", async () => {
      await renderedRows([docs, inbox]);
      expect(within(rowOf("Inbox")).getByText(/3 spans/)).toBeInTheDocument();
      expect(within(rowOf("Docs")).queryByText(/spans/)).not.toBeInTheDocument();
    });

    it("shows how much of a row is recorded", async () => {
      await renderedRows([todo]);
      expect(within(rowOf("Todo")).getByTitle("0:10 recorded")).toBeInTheDocument();
    });

    it("says so when the day has no activity", async () => {
      renderList([]);
      expect(await screen.findByText("No activity on this day")).toBeInTheDocument();
    });

    it("shows the error when the list cannot be loaded", async () => {
      mocked.listView.mockRejectedValue({ kind: "db", message: "Database locked", code: null });
      mocked.dayView.mockResolvedValue(dayView({ lanes: [lane({ blocks: [block()] })] }));
      resetStore({ day: DAY, permissionDialogHidden: true, settings: settings(), uiState: { ...EMPTY_UI_STATE, dayMode: "list" } });
      render(<DayPage />);
      expect(await screen.findByText("Database locked")).toBeInTheDocument();
    });

    it("loads the list again when the day's recordings changed", async () => {
      await renderedRows([todo]);
      expect(within(rowOf("Todo")).getByTitle("0:10 recorded")).toBeInTheDocument();
      mocked.listView.mockResolvedValue(listView({ rows: [{ ...todo, coveredMs: 30 * MINUTE_MS }] }));
      act(() => useStore.getState().dataChanged({ kind: "recordings", day: DAY }));
      expect(await screen.findByTitle("0:30 recorded")).toBeInTheDocument();
    });

    it("opens the details of a row when its title is clicked and closes them with another click", async () => {
      await renderedRows([docs]);
      await userEvent.click(within(rowOf("Docs")).getByRole("button", { name: "Docs" }));
      const row = within(rowOf("Docs"));
      expect(row.getByText("URL")).toBeInTheDocument();
      expect(row.getByText("09:00–10:00")).toBeInTheDocument();
      await userEvent.click(row.getByRole("button", { name: "Docs" }));
      expect(row.queryByText("URL")).not.toBeInTheDocument();
    });
  });

  describe("search", () => {
    it.each([
      ["title", "todo", ["Todo"]],
      ["app", "safari", ["Docs"]],
      ["url", "example.com", ["Docs"]],
      ["path", "/tmp", ["Report.pdf"]],
    ])("filters the rows by %s", async (_field, query, titles) => {
      await renderedRows([docs, todo, report]);
      await userEvent.type(search(), query);
      expect(rowEls().map((r) => within(r).getByRole("button").textContent)).toEqual(titles);
    });

    it("says nothing matches when no row does", async () => {
      await renderedRows([docs]);
      await userEvent.type(search(), "zzz");
      expect(screen.getByText("No activity matches")).toBeInTheDocument();
    });

    it("clears the search with Escape", async () => {
      await renderedRows([docs, todo]);
      await userEvent.type(search(), "todo");
      await userEvent.keyboard("{Escape}");
      expect(search()).toHaveValue("");
      expect(rowEls()).toHaveLength(2);
    });

    it("focuses the search with /", async () => {
      await renderedRows([docs]);
      await userEvent.keyboard("/");
      expect(search()).toHaveFocus();
      expect(search()).toHaveValue("");
    });
  });

  describe("dropping a row on a project", () => {
    it("opens the editor with the row's span and the project", async () => {
      await renderedRows([docs, todo]);
      dragRow(rowOf("Docs"), chip("Website"));
      const dialog = within(await editorDialog());
      expect(dialog.getByLabelText("From")).toHaveValue("09:00");
      expect(dialog.getByLabelText("To")).toHaveValue("10:00");
      expect(dialog.getByRole("combobox", { name: "Project" })).toHaveValue("Website");
      expect(dialog.queryByRole("note")).not.toBeInTheDocument();
    });

    it("warns about the gaps of a row with several spans", async () => {
      await renderedRows([inbox]);
      dragRow(rowOf("Inbox"), chip("Website"));
      const dialog = within(await editorDialog());
      expect(dialog.getByRole("note")).toHaveTextContent("This activity has gaps (3 spans, 0:30 in between). Adjust the times if needed.");
      expect(dialog.getByLabelText("From")).toHaveValue("13:00");
      expect(dialog.getByLabelText("To")).toHaveValue("14:20");
    });

    it("creates the recording with overlap cancel on Save", async () => {
      await renderedRows([docs]);
      dragRow(rowOf("Docs"), chip("Website"));
      await editorDialog();
      await userEvent.click(screen.getByRole("button", { name: "Save" }));
      await waitFor(() =>
        expect(mocked.recordingCreate).toHaveBeenCalledWith({ projectId: "p-site", start: at("09:00"), end: at("10:00"), note: "", tagId: null }, "cancel"),
      );
    });

    it("opens no editor when released away from the projects", async () => {
      await renderedRows([docs]);
      dragRow(rowOf("Docs"), null);
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("opens no editor for a press that was not moved", async () => {
      await renderedRows([docs]);
      fireEvent.pointerDown(rowOf("Docs"), { button: 0, clientX: 10, clientY: 10 });
      fireEvent.pointerUp(rowOf("Docs"), { clientX: 10, clientY: 10 });
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    describe("with more projects than the strip shows", () => {
      const many = Array.from({ length: 9 }, (_, i) => projectItem({ project: { id: `p${i + 1}`, name: `Project ${i + 1}` } }));
      const manyProjects = projectsStore({ projects: many });

      it("offers the rest in the More… menu, which records the dragged row", async () => {
        await renderedRows([docs], manyProjects);
        expect(screen.queryByRole("button", { name: "Project 9" })).not.toBeInTheDocument();
        dragRow(rowOf("Docs"), screen.getByRole("button", { name: "More…" }));
        await userEvent.click(await screen.findByRole("menuitem", { name: "Project 9" }));
        const dialog = within(await editorDialog());
        expect(dialog.getByRole("combobox", { name: "Project" })).toHaveValue("Project 9");
        expect(dialog.getByLabelText("From")).toHaveValue("09:00");
      });
    });
  });

  describe("recording the focused row", () => {
    it("records the focused row for the project of a clicked chip", async () => {
      await renderedRows([docs, todo]);
      await userEvent.click(rowOf("Todo"));
      await userEvent.click(chip("Website"));
      const dialog = within(await editorDialog());
      expect(dialog.getByLabelText("From")).toHaveValue("10:00");
      expect(dialog.getByLabelText("To")).toHaveValue("10:30");
      expect(dialog.getByRole("combobox", { name: "Project" })).toHaveValue("Website");
    });

    it("opens nothing for a chip click while no row is focused", async () => {
      await renderedRows([docs]);
      await userEvent.click(chip("Website"));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("says there are no projects yet when there are none", async () => {
      await renderedRows([docs], projectsStore());
      expect(screen.getByText("No projects yet")).toBeInTheDocument();
    });
  });

  describe("keyboard", () => {
    it("opens the editor for the row on Enter, with the project picker focused and no project", async () => {
      await renderedRows([docs, todo]);
      rowOf("Todo").focus();
      await userEvent.keyboard("{Enter}");
      const dialog = within(await editorDialog());
      expect(dialog.getByLabelText("From")).toHaveValue("10:00");
      expect(dialog.getByLabelText("To")).toHaveValue("10:30");
      const picker = dialog.getByRole("combobox", { name: "Project" });
      expect(picker).toHaveValue("");
      expect(picker).toHaveFocus();
    });

    it("moves between the rows with ↓ and ↑ and stops at the last", async () => {
      await renderedRows([docs, todo, report]);
      rowOf("Docs").focus();
      await userEvent.keyboard("{ArrowDown}");
      expect(rowOf("Todo")).toHaveFocus();
      await userEvent.keyboard("{ArrowDown}{ArrowDown}");
      expect(rowOf("Report.pdf")).toHaveFocus();
      await userEvent.keyboard("{ArrowUp}");
      expect(rowOf("Todo")).toHaveFocus();
    });

    it("moves from the first row up to the search field", async () => {
      await renderedRows([docs, todo]);
      rowOf("Docs").focus();
      await userEvent.keyboard("{ArrowUp}");
      expect(search()).toHaveFocus();
    });

    it("selects the row's span and shows the timeline on Space", async () => {
      await renderedRows([docs, todo]);
      rowOf("Docs").focus();
      await userEvent.keyboard(" ");
      expect(useStore.getState().selection).toEqual({ start: docs.firstStart, end: docs.lastEnd, blockIds: ["a1"] });
      expect(screen.queryByRole("list", { name: "Activity" })).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Timeline" })).toHaveAttribute("aria-pressed", "true");
    });
  });
});

describe("Day mode", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    window.addEventListener("keydown", handleKeydown, true);
  });

  afterEach(() => {
    window.removeEventListener("keydown", handleKeydown, true);
    flushUiState();
    vi.useRealTimers();
  });

  const modeButton = (name: "Timeline" | "List") => within(screen.getByRole("group", { name: "Day mode" })).getByRole("button", { name });

  it("shows the timeline by default and the list after List is clicked", async () => {
    renderList([docs], { uiState: EMPTY_UI_STATE });
    expect(modeButton("Timeline")).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(modeButton("List"));
    expect(modeButton("List")).toHaveAttribute("aria-pressed", "true");
    expect(await activity()).toBeInTheDocument();
  });

  it("writes the chosen mode to the ui state", async () => {
    renderList([docs], { uiState: EMPTY_UI_STATE });
    await userEvent.click(modeButton("List"));
    await act(() => vi.advanceTimersByTimeAsync(600));
    expect(mocked.setUiState).toHaveBeenCalledWith(expect.objectContaining({ dayMode: "list" }));
  });

  it("toggles between the timeline and the list with ⌘⇧L", async () => {
    renderList([docs], { uiState: EMPTY_UI_STATE });
    await userEvent.keyboard("{Meta>}{Shift>}l{/Shift}{/Meta}");
    expect(modeButton("List")).toHaveAttribute("aria-pressed", "true");
    expect(await activity()).toBeInTheDocument();
    await userEvent.keyboard("{Meta>}{Shift>}l{/Shift}{/Meta}");
    expect(modeButton("Timeline")).toHaveAttribute("aria-pressed", "true");
    expect(screen.queryByRole("list", { name: "Activity" })).not.toBeInTheDocument();
  });

  it("toggles with the day.toggleList command", async () => {
    renderList([docs], { uiState: EMPTY_UI_STATE });
    act(() => void runCommand("day.toggleList"));
    expect(await activity()).toBeInTheDocument();
  });

  it.each(["day.nextBlock", "day.prevBlock", "day.nextLane", "day.prevLane"])("%s is unavailable in the list", (id) => {
    renderList([docs]);
    expect(runCommand(id)).toBe(false);
  });

  it("recording.open is unavailable in the list even with a selection", async () => {
    renderList([docs]);
    await activity();
    act(() => useStore.getState().setSelection({ start: at("09:00"), end: at("09:30"), blockIds: [] }));
    expect(runCommand("recording.open")).toBe(false);
  });
});
