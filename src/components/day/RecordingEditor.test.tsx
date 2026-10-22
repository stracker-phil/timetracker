import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { at, dayView, recordingBlock } from "../../test/dayFixtures";
import { client, projectItem, projectsStore } from "../../test/projectFixtures";
import { recording } from "../../test/recordingFixtures";
import { settings } from "../../test/settings";
import { Dialogs } from "../Dialogs";
import { Toasts } from "../Toasts";
import { RecordingEditor } from "./RecordingEditor";

const mocked = vi.mocked(api, true);

const acme = client({ id: "c-acme", name: "Acme" });
const website = projectItem({ project: { id: "p-site", name: "Website", clientId: acme.id }, client: acme });
const START = at("09:00");
const END = at("10:30");
const standupBlock = recordingBlock({ id: "r1", projectId: "p-site", note: "standup", start: at("11:00"), end: at("11:45") });
const standupRow = recording({ id: "r1", projectId: "p-site", note: "standup", tagId: "tag-research", start: at("11:00"), end: at("11:45") });

/** Shows the editor the store asks for, like `DayPage` does. */
function Host() {
  const editor = useStore((s) => s.editor);
  return editor && <RecordingEditor editor={editor} />;
}

function renderEditor(editor: NonNullable<ReturnType<typeof useStore.getState>["editor"]>, patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore({ settings: settings(), ...projectsStore({ projects: [website], clients: [acme] }), dayView: dayView({ recordings: [standupBlock] }), editor, ...patch });
  render(
    <>
      <Host />
      <Dialogs />
      <Toasts />
    </>,
  );
}

const renderCreate = (patch: Parameters<typeof resetStore>[0] = {}) => renderEditor({ mode: "create", start: START, end: END }, patch);
const editorDialog = (name: "New recording" | "Edit recording") => screen.findByRole("dialog", { name });
const field = (label: string) => screen.getByLabelText(label);
const projectField = () => screen.getByRole("combobox", { name: "Project" });
const tagField = () => screen.getByRole("combobox", { name: "Tag" });
const button = (name: string) => screen.getByRole("button", { name });

async function chooseWebsite() {
  await userEvent.type(projectField(), "Web{Enter}");
}

describe("RecordingEditor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.recordingsList.mockResolvedValue([]);
  });

  describe("creating", () => {
    it("shows the span with its length", async () => {
      renderCreate();
      await editorDialog("New recording");
      expect(field("From")).toHaveValue("09:00");
      expect(field("To")).toHaveValue("10:30");
      expect(screen.getByText("1 h 30 min")).toBeInTheDocument();
    });

    it("saves the project, times, note and tag with overlap cancel, then closes and offers Undo", async () => {
      renderCreate();
      await editorDialog("New recording");
      await chooseWebsite();
      await userEvent.type(field("Note"), "fix login");
      await userEvent.type(tagField(), "res{Enter}");
      await userEvent.click(button("Save"));
      await waitFor(() =>
        expect(mocked.recordingCreate).toHaveBeenCalledWith({ projectId: "p-site", spans: [{ start: START, end: END }], note: "fix login", tagId: "tag-research" }, "cancel"),
      );
      expect(await screen.findByText("Recorded 1 h 30 min for Website")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
      expect(screen.queryByRole("dialog", { name: "New recording" })).not.toBeInTheDocument();
    });

    it("saves with Enter in the note field", async () => {
      renderCreate();
      await editorDialog("New recording");
      await chooseWebsite();
      await userEvent.type(field("Note"), "standup{Enter}");
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ note: "standup" }), "cancel"));
    });

    it("saves with ⌘Enter from any field", async () => {
      renderCreate();
      await editorDialog("New recording");
      await chooseWebsite();
      await userEvent.click(field("From"));
      await userEvent.keyboard("{Meta>}{Enter}{/Meta}");
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ projectId: "p-site" }), "cancel"));
    });

    it("asks for a project and saves nothing while none is chosen", async () => {
      renderCreate();
      await editorDialog("New recording");
      await userEvent.click(button("Save"));
      expect(await screen.findByRole("alert")).toHaveTextContent("Pick a project");
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });

    it("turns a pasted line break in the note into a space", async () => {
      renderCreate();
      await editorDialog("New recording");
      await userEvent.click(field("Note"));
      await userEvent.paste("fix\nlogin\r\nbug");
      expect(field("Note")).toHaveValue("fix login bug");
    });

    it("shows the error of the field Rust rejected and stays open", async () => {
      mocked.recordingCreate.mockRejectedValueOnce({ kind: "invalid_input", message: "`note`: too long", code: null, field: { path: "note", message: "The note is too long" } });
      renderCreate();
      await editorDialog("New recording");
      await chooseWebsite();
      await userEvent.click(button("Save"));
      expect(await screen.findByText("The note is too long")).toBeInTheDocument();
      expect(screen.getByRole("dialog", { name: "New recording" })).toBeInTheDocument();
    });

    it("toasts an error that belongs to no field and stays open", async () => {
      mocked.recordingCreate.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      renderCreate();
      await editorDialog("New recording");
      await chooseWebsite();
      await userEvent.click(button("Save"));
      expect(await screen.findByText("Could not save the recording: Database locked")).toBeInTheDocument();
      expect(screen.getByRole("dialog", { name: "New recording" })).toBeInTheDocument();
    });

    it("closes with Cancel and with Escape without saving", async () => {
      renderCreate();
      await editorDialog("New recording");
      await userEvent.click(button("Cancel"));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      useStore.getState().openEditor({ mode: "create", start: START, end: END });
      await editorDialog("New recording");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });

    it("has no Delete button", async () => {
      renderCreate();
      await editorDialog("New recording");
      expect(screen.queryByRole("button", { name: "Delete" })).not.toBeInTheDocument();
    });
  });

  describe("times", () => {
    it("commits a typed time on blur and updates the length", async () => {
      renderCreate();
      await editorDialog("New recording");
      await userEvent.clear(field("To"));
      await userEvent.type(field("To"), "11:15");
      await userEvent.tab();
      expect(field("To")).toHaveValue("11:15");
      expect(screen.getByText("2 h 15 min")).toBeInTheDocument();
    });

    it("counts a valid typed time at once, so Enter saves it without leaving the field", async () => {
      renderCreate();
      await editorDialog("New recording");
      await chooseWebsite();
      await userEvent.clear(field("To"));
      await userEvent.type(field("To"), "11:15{Enter}");
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [{ start: START, end: at("11:15") }] }), "cancel"));
    });

    it("updates the length while a valid time is typed, before the field is left", async () => {
      renderCreate();
      await editorDialog("New recording");
      await userEvent.clear(field("To"));
      await userEvent.type(field("To"), "11:15");
      expect(screen.getByText("2 h 15 min")).toBeInTheDocument();
    });

    it("accepts a time typed without a colon", async () => {
      renderCreate();
      await editorDialog("New recording");
      await userEvent.clear(field("From"));
      await userEvent.type(field("From"), "0845");
      await userEvent.tab();
      expect(field("From")).toHaveValue("08:45");
    });

    it("restores the previous time when the text is not a time", async () => {
      renderCreate();
      await editorDialog("New recording");
      await userEvent.clear(field("To"));
      await userEvent.type(field("To"), "later");
      await userEvent.tab();
      expect(field("To")).toHaveValue("10:30");
    });

    it("steps a time by the snap interval with the arrow keys", async () => {
      renderCreate({ settings: settings({ timeline: { ...settings().timeline, snapMinutes: 15 } }) });
      await editorDialog("New recording");
      await userEvent.type(field("To"), "{ArrowUp}");
      expect(field("To")).toHaveValue("10:45");
      await userEvent.type(field("From"), "{ArrowDown}{ArrowDown}");
      expect(field("From")).toHaveValue("08:30");
    });

    it("takes an end before the start as the next day", async () => {
      renderCreate();
      await editorDialog("New recording");
      await chooseWebsite();
      await userEvent.clear(field("To"));
      await userEvent.type(field("To"), "08:00");
      await userEvent.tab();
      expect(screen.getByText("23 h 00 min")).toBeInTheDocument();
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [{ start: START, end: at("08:00", "2026-03-11") }] }), "cancel"));
    });
  });

  describe("overlapping another recording", () => {
    const inTheWay = recording({ id: "r9", projectId: "p-site", note: "fix login", start: at("09:15"), end: at("09:45") });

    async function saveIntoOverlap() {
      mocked.recordingCreate.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps a recording", code: null, details: [inTheWay] });
      renderCreate();
      await editorDialog("New recording");
      await chooseWebsite();
      await userEvent.click(button("Save"));
      return within(await screen.findByRole("dialog", { name: "This overlaps a recording" }));
    }

    it("lists the recordings in the way", async () => {
      const dialog = await saveIntoOverlap();
      expect(dialog.getByRole("listitem")).toHaveTextContent("09:15–09:45 Website · fix login");
    });

    it("lists every span of a recording in the way", async () => {
      const sittings = recording({
        ...inTheWay,
        spans: [
          { id: "r9-s1", start: at("09:15"), end: at("09:45") },
          { id: "r9-s2", start: at("16:05"), end: at("20:15") },
        ],
      });
      mocked.recordingCreate.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps a recording", code: null, details: [sittings] });
      renderCreate();
      await editorDialog("New recording");
      await chooseWebsite();
      await userEvent.click(button("Save"));
      const dialog = within(await screen.findByRole("dialog", { name: "This overlaps a recording" }));
      expect(dialog.getByRole("listitem")).toHaveTextContent("09:15–09:45, 16:05–20:15 Website · fix login");
    });

    it("saves again with overlap replace when Replace is chosen, then closes the editor", async () => {
      const dialog = await saveIntoOverlap();
      await userEvent.click(dialog.getByRole("button", { name: "Replace overlapped part" }));
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenLastCalledWith({ projectId: "p-site", spans: [{ start: START, end: END }], note: "", tagId: null }, "replace"));
      await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    });

    it("saves nothing more when Cancel is chosen and keeps the editor open", async () => {
      const dialog = await saveIntoOverlap();
      await userEvent.click(dialog.getByRole("button", { name: "Cancel" }));
      await waitFor(() => expect(screen.queryByRole("dialog", { name: "This overlaps a recording" })).not.toBeInTheDocument());
      expect(mocked.recordingCreate).not.toHaveBeenCalledWith(expect.anything(), "replace");
      expect(screen.getByRole("dialog", { name: "New recording" })).toBeInTheDocument();
    });
  });

  describe("editing", () => {
    beforeEach(() => mocked.recordingsList.mockResolvedValue([standupRow]));

    it("loads the whole recording of the block and shows its fields", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorDialog("Edit recording");
      expect(mocked.recordingsList).toHaveBeenCalledWith(standupBlock.start, standupBlock.end);
      expect(field("From")).toHaveValue("11:00");
      expect(field("To")).toHaveValue("11:45");
      expect(field("Note")).toHaveValue("standup");
      await userEvent.tab();
      expect(projectField()).toHaveValue("Acme / Website");
    });

    it("shows the tag of the recording", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorDialog("Edit recording");
      expect(tagField()).toHaveValue("research");
    });

    it("sends tagId null when the tag is cleared with Backspace", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorDialog("Edit recording");
      await userEvent.click(tagField());
      await userEvent.keyboard("{Backspace}");
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", expect.objectContaining({ tagId: null }), "cancel"));
    });

    it("sends tagId null when No tag is picked", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorDialog("Edit recording");
      await userEvent.click(tagField());
      await userEvent.click(screen.getByRole("option", { name: "No tag" }));
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", expect.objectContaining({ tagId: null }), "cancel"));
    });

    it("shows the times of a recording that was cut to the day", async () => {
      const longer = recording({ ...standupRow, start: at("22:00", "2026-03-09"), end: at("01:00") });
      mocked.recordingsList.mockResolvedValue([longer]);
      renderEditor({ mode: "edit", id: "r1" }, { dayView: dayView({ recordings: [{ ...standupBlock, start: at("00:00"), end: at("01:00"), clippedStart: true }] }) });
      await editorDialog("Edit recording");
      expect(field("From")).toHaveValue("22:00");
      expect(field("To")).toHaveValue("01:00");
      expect(screen.getByText("3 h 00 min")).toBeInTheDocument();
    });

    it("saves the changes to the recording with overlap cancel", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorDialog("Edit recording");
      await userEvent.type(field("Note"), " notes");
      await userEvent.click(button("Save"));
      await waitFor(() =>
        expect(mocked.recordingUpdate).toHaveBeenCalledWith(
          "r1",
          { projectId: "p-site", spans: [{ start: standupBlock.start, end: standupBlock.end }], note: "standup notes", tagId: "tag-research" },
          "cancel",
        ),
      );
      expect(await screen.findByText("Recording changed")).toBeInTheDocument();
    });

    it("deletes the recording with Delete and closes", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorDialog("Edit recording");
      await userEvent.click(button("Delete"));
      await waitFor(() => expect(mocked.recordingDelete).toHaveBeenCalledWith("r1"));
      expect(screen.queryByRole("dialog", { name: "Edit recording" })).not.toBeInTheDocument();
      expect(await screen.findByText("Recording deleted")).toBeInTheDocument();
    });

    it("deletes the recording with ⌘Backspace", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorDialog("Edit recording");
      await userEvent.keyboard("{Meta>}{Backspace}{/Meta}");
      await waitFor(() => expect(mocked.recordingDelete).toHaveBeenCalledWith("r1"));
    });

    describe("with several spans", () => {
      const morning = { id: "r1-s1", start: at("09:00"), end: at("09:30") };
      const noon = { id: "r1-s2", start: at("11:00"), end: at("11:45") };
      const evening = { id: "r1-s3", start: at("16:00"), end: at("17:00") };
      const sittings = recording({ ...standupRow, spans: [morning, noon, evening] });
      const noonBlock = { ...standupBlock, spanId: noon.id, spanCount: 3 };

      beforeEach(() => mocked.recordingsList.mockResolvedValue([sittings]));

      it("shows the span of the opened block and lists the other spans as Also", async () => {
        renderEditor({ mode: "edit", id: "r1" }, { dayView: dayView({ recordings: [noonBlock] }) });
        await editorDialog("Edit recording");
        expect(field("From")).toHaveValue("11:00");
        expect(field("To")).toHaveValue("11:45");
        expect(screen.getByText("Also 09:00–09:30, 16:00–17:00")).toBeInTheDocument();
      });

      it("shows the span the editor was opened with", async () => {
        renderEditor({ mode: "edit", id: "r1", spanId: evening.id }, { dayView: dayView({ recordings: [noonBlock] }) });
        await editorDialog("Edit recording");
        expect(field("From")).toHaveValue("16:00");
        expect(field("To")).toHaveValue("17:00");
        expect(screen.getByText("Also 09:00–09:30, 11:00–11:45")).toBeInTheDocument();
      });

      it("saves all spans with only the edited one changed", async () => {
        renderEditor({ mode: "edit", id: "r1" }, { dayView: dayView({ recordings: [noonBlock] }) });
        await editorDialog("Edit recording");
        await userEvent.clear(field("To"));
        await userEvent.type(field("To"), "12:00");
        await userEvent.click(button("Save"));
        await waitFor(() =>
          expect(mocked.recordingUpdate).toHaveBeenCalledWith(
            "r1",
            expect.objectContaining({
              spans: [
                { start: morning.start, end: morning.end },
                { start: noon.start, end: at("12:00") },
                { start: evening.start, end: evening.end },
              ],
            }),
            "cancel",
          ),
        );
      });

      it("asks to stop the timer first when a span is still open, and saves nothing", async () => {
        const running = recording({ ...standupRow, spans: [morning, { id: "r1-s2", start: at("11:00"), end: null }] });
        mocked.recordingsList.mockResolvedValue([running]);
        renderEditor({ mode: "edit", id: "r1", spanId: morning.id }, { dayView: dayView({ recordings: [{ ...standupBlock, spanId: morning.id, spanCount: 2 }] }) });
        await editorDialog("Edit recording");
        await userEvent.click(button("Save"));
        expect(await screen.findByText("Stop the timer first")).toBeInTheDocument();
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
        expect(screen.getByRole("dialog", { name: "Edit recording" })).toBeInTheDocument();
      });

      it("shows no Also line for a recording with one span", async () => {
        mocked.recordingsList.mockResolvedValue([standupRow]);
        renderEditor({ mode: "edit", id: "r1" });
        await editorDialog("Edit recording");
        expect(screen.queryByText(/^Also /)).not.toBeInTheDocument();
      });
    });

    it("shows nothing and an error when the recording cannot be loaded", async () => {
      mocked.recordingsList.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      renderEditor({ mode: "edit", id: "r1" });
      expect(await screen.findByText("Could not load the recording: Database locked")).toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });
});
