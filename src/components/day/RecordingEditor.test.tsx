import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import type { Editor } from "../../state/slices/editor";
import { resetStore, useStore } from "../../state/store";
import { at, dayView, recordingBlock } from "../../test/dayFixtures";
import { client, projectItem, projectsStore } from "../../test/projectFixtures";
import { recording } from "../../test/recordingFixtures";
import { settings } from "../../test/settings";
import { Dialogs } from "../Dialogs";
import { Toasts } from "../Toasts";
import { RecordingEditor } from "./RecordingEditor";

const mocked = vi.mocked(api, true);

const MINUTE_MS = 60_000;
const acme = client({ id: "c-acme", name: "Acme" });
const website = projectItem({ project: { id: "p-site", name: "Website", clientId: acme.id }, client: acme });
const intranet = projectItem({ project: { id: "p-intra", name: "Intranet" } });
const START = at("09:00");
const END = at("10:30");
const standupBlock = recordingBlock({ id: "r1", projectId: "p-site", note: "standup", start: at("11:00"), end: at("11:45") });
const standupRow = recording({ id: "r1", projectId: "p-site", note: "standup", tagId: "tag-research", start: at("11:00"), end: at("11:45") });

function renderEditor(editor: Editor, patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore({ settings: settings(), ...projectsStore({ projects: [website, intranet], clients: [acme] }), dayView: dayView({ recordings: [standupBlock] }), ...patch });
  act(() => useStore.getState().openEditor(editor));
  render(
    <>
      <RecordingEditor />
      <Dialogs />
      <Toasts />
    </>,
  );
}

const renderCreate = (patch: Parameters<typeof resetStore>[0] = {}) => renderEditor({ mode: "create", start: START, end: END }, patch);
const editorForm = (name: "New recording" | "Edit recording") => screen.findByRole("form", { name });
const field = (label: string) => screen.getByLabelText(label);
const projectField = () => screen.getByRole("combobox", { name: "Project" });
const tagField = () => screen.getByRole("combobox", { name: "Tag" });
const button = (name: string | RegExp) => screen.getByRole("button", { name });

async function chooseWebsite() {
  await userEvent.type(projectField(), "Web{Enter}");
}

describe("RecordingEditor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.recordingsList.mockResolvedValue([]);
    mocked.recordingProposal.mockResolvedValue({ projectId: null });
  });

  describe("creating", () => {
    it("shows the span as a From and To row with its length", async () => {
      renderCreate();
      await editorForm("New recording");
      expect(field("Span 1 from")).toHaveValue("09:00");
      expect(field("Span 1 to")).toHaveValue("10:30");
      expect(field("Hours")).toHaveValue("1");
      expect(field("Minutes")).toHaveValue("30");
    });

    it("labels the span row From and to", async () => {
      renderCreate();
      const form = within(await editorForm("New recording"));
      expect(form.getByText("From")).toBeInTheDocument();
      expect(form.getByText("to")).toBeInTheDocument();
    });

    it("focuses the note", async () => {
      renderCreate();
      await editorForm("New recording");
      expect(field("Note")).toHaveFocus();
    });

    it("saves the project, spans, note and tag with overlap cancel, then closes and offers Undo", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.type(field("Note"), "fix login");
      await userEvent.type(tagField(), "res{Enter}");
      await userEvent.click(button("Save"));
      await waitFor(() =>
        expect(mocked.recordingCreate).toHaveBeenCalledWith({ projectId: "p-site", spans: [{ start: START, end: END }], note: "fix login", tagId: "tag-research" }, "cancel"),
      );
      expect(await screen.findByText("Recorded 1 h 30 min for Website")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
    });

    it("saves with Enter in the note field", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.type(field("Note"), "standup{Enter}");
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ note: "standup" }), "cancel"));
    });

    it("saves with ⌘Enter from any field", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.click(field("Span 1 from"));
      await userEvent.keyboard("{Meta>}{Enter}{/Meta}");
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ projectId: "p-site" }), "cancel"));
    });

    it("asks for a project and saves nothing while none is chosen", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.click(button("Save"));
      expect(await screen.findByRole("alert")).toHaveTextContent("Pick a project");
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });

    it("turns a pasted line break in the note into a space", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.click(field("Note"));
      await userEvent.paste("fix\nlogin\r\nbug");
      expect(field("Note")).toHaveValue("fix login bug");
    });

    it("shows the error of the field Rust rejected and stays open", async () => {
      mocked.recordingCreate.mockRejectedValueOnce({ kind: "invalid_input", message: "`note`: too long", code: null, field: { path: "note", message: "The note is too long" } });
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.click(button("Save"));
      expect(await screen.findByText("The note is too long")).toBeInTheDocument();
      expect(screen.getByRole("form", { name: "New recording" })).toBeInTheDocument();
    });

    it("toasts an error that belongs to no field and stays open", async () => {
      mocked.recordingCreate.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.click(button("Save"));
      expect(await screen.findByText("Could not save the recording: Database locked")).toBeInTheDocument();
      expect(screen.getByRole("form", { name: "New recording" })).toBeInTheDocument();
    });

    it("closes with Cancel without saving", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.click(button("Cancel"));
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });

    it("closes with Escape and clears the draft and the selection", async () => {
      renderCreate();
      await editorForm("New recording");
      act(() => useStore.getState().setSelection({ start: START, end: END, blockIds: [] }));
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
      expect(useStore.getState().draft).toBeNull();
      expect(useStore.getState().selection).toBeNull();
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });

    it("has no Delete button", async () => {
      renderCreate();
      await editorForm("New recording");
      expect(screen.queryByRole("button", { name: "Delete" })).not.toBeInTheDocument();
    });
  });

  describe("proposal", () => {
    it("asks Rust about the spans of the draft", async () => {
      renderCreate();
      await editorForm("New recording");
      await waitFor(() => expect(mocked.recordingProposal).toHaveBeenCalledWith([{ start: START, end: END }]));
    });

    it("prefills the project of a new recording from the proposal", async () => {
      mocked.recordingProposal.mockResolvedValue({ projectId: "p-site" });
      renderCreate();
      await editorForm("New recording");
      await waitFor(() => expect(projectField()).toHaveValue("Acme / Website"));
    });

    it("leaves a project the opener chose alone", async () => {
      mocked.recordingProposal.mockResolvedValue({ projectId: "p-site" });
      renderEditor({ mode: "create", start: START, end: END, projectId: "p-intra" });
      await editorForm("New recording");
      await waitFor(() => expect(mocked.recordingProposal).toHaveBeenCalled());
      expect(projectField()).toHaveValue("Intranet");
    });

    it("asks again for the new spans when a time changes", async () => {
      renderCreate();
      await editorForm("New recording");
      await waitFor(() => expect(mocked.recordingProposal).toHaveBeenCalledWith([{ start: START, end: END }]));
      await userEvent.clear(field("Span 1 to"));
      await userEvent.type(field("Span 1 to"), "11:15");
      await waitFor(() => expect(mocked.recordingProposal).toHaveBeenLastCalledWith([{ start: START, end: at("11:15") }]));
    });

    it("has no Activity button", async () => {
      renderCreate();
      await editorForm("New recording");
      await waitFor(() => expect(mocked.recordingProposal).toHaveBeenCalled());
      expect(screen.queryByRole("button", { name: /^Activity/ })).not.toBeInTheDocument();
    });
  });

  describe("pickers", () => {
    it("keeps the editor open when Escape closes an open picker list", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.click(projectField());
      await userEvent.keyboard("{ArrowDown}");
      expect(screen.getByRole("listbox", { name: "Projects" })).toBeInTheDocument();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      expect(screen.getByRole("form", { name: "New recording" })).toBeInTheDocument();
    });

    it("saves with Enter in a picker whose list is closed", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.keyboard("{Enter}");
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ projectId: "p-site" }), "cancel"));
    });

    it("closes the editor with Escape in a picker whose list is closed", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.click(tagField());
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
    });
  });

  describe("times", () => {
    it("commits a typed time on blur and updates the length", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.clear(field("Span 1 to"));
      await userEvent.type(field("Span 1 to"), "11:15");
      await userEvent.tab();
      expect(field("Span 1 to")).toHaveValue("11:15");
      await waitFor(() => expect(field("Hours")).toHaveValue("2"));
      expect(field("Minutes")).toHaveValue("15");
    });

    it("asks Rust about the spans once a valid time is typed", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.clear(field("Span 1 to"));
      await userEvent.type(field("Span 1 to"), "11:15");
      await waitFor(() => expect(mocked.recordingDraftSpans).toHaveBeenLastCalledWith([{ start: START, end: at("11:15") }]));
    });

    it("counts a valid typed time at once, so Enter saves it without leaving the field", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.clear(field("Span 1 to"));
      await userEvent.type(field("Span 1 to"), "11:15{Enter}");
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [{ start: START, end: at("11:15") }] }), "cancel"));
    });

    it("updates the length while a valid time is typed, before the field is left", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.clear(field("Span 1 to"));
      await userEvent.type(field("Span 1 to"), "11:15");
      await waitFor(() => expect(field("Hours")).toHaveValue("2"));
      expect(field("Minutes")).toHaveValue("15");
    });

    it("accepts a time typed without a colon", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.clear(field("Span 1 from"));
      await userEvent.type(field("Span 1 from"), "0845");
      await userEvent.tab();
      expect(field("Span 1 from")).toHaveValue("08:45");
    });

    it("restores the previous time when the text is not a time", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.clear(field("Span 1 to"));
      await userEvent.type(field("Span 1 to"), "later");
      await userEvent.tab();
      expect(field("Span 1 to")).toHaveValue("10:30");
    });

    it("steps a time by the snap interval with the arrow keys", async () => {
      renderCreate({ settings: settings({ timeline: { ...settings().timeline, snapMinutes: 15 } }) });
      await editorForm("New recording");
      await userEvent.type(field("Span 1 to"), "{ArrowUp}");
      expect(field("Span 1 to")).toHaveValue("10:45");
      await userEvent.type(field("Span 1 from"), "{ArrowDown}{ArrowDown}");
      expect(field("Span 1 from")).toHaveValue("08:30");
    });

    it("sends an end before the start as typed and shows the next-day end Rust rolls it to", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      const rolled = { start: START, end: at("08:00", "2026-03-11") };
      mocked.recordingDraftSpans.mockResolvedValueOnce({ spans: [rolled], totalMs: rolled.end - rolled.start });
      await userEvent.clear(field("Span 1 to"));
      await userEvent.type(field("Span 1 to"), "08:00");
      await waitFor(() => expect(mocked.recordingDraftSpans).toHaveBeenLastCalledWith([{ start: START, end: at("08:00") }]));
      await waitFor(() => expect(field("Hours")).toHaveValue("23"));
      expect(field("Minutes")).toHaveValue("0");
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [rolled] }), "cancel"));
    });
  });

  describe("saving while Rust is still answering for a typed time", () => {
    afterEach(() => void mocked.calendarParseTime.mockReset());

    async function openWithProject() {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
    }

    it.each([
      ["Enter", "{Enter}"],
      ["⌘Enter", "{Meta>}{Enter}{/Meta}"],
    ])("%s waits for Rust to read the time and saves it", async (_name, keys) => {
      await openWithProject();
      const readings: ((ms: number | null) => void)[] = [];
      mocked.calendarParseTime.mockImplementation(() => new Promise((resolve) => readings.push(resolve)));
      await userEvent.clear(field("Span 1 to"));
      await userEvent.type(field("Span 1 to"), `11:15${keys}`);
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
      act(() => readings.forEach((read) => read(at("11:15"))));
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [{ start: START, end: at("11:15") }] }), "cancel"));
    });

    it("saves the new start time of an existing recording once Rust has read it", async () => {
      mocked.recordingsList.mockResolvedValue([standupRow]);
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      const readings: ((ms: number | null) => void)[] = [];
      mocked.calendarParseTime.mockImplementation(() => new Promise((resolve) => readings.push(resolve)));
      await userEvent.clear(field("Span 1 from"));
      await userEvent.type(field("Span 1 from"), "1{Enter}");
      expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      act(() => readings.forEach((read) => read(at("10:45"))));
      await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", expect.objectContaining({ spans: [{ start: at("10:45"), end: standupBlock.end }] }), "cancel"));
    });

    it("waits for Rust's answer about the spans and saves what it answered", async () => {
      await openWithProject();
      const rolled = { start: START, end: at("08:00", "2026-03-11") };
      let answer: (draft: { spans: { start: number; end: number }[]; totalMs: number }) => void = () => {};
      mocked.recordingDraftSpans.mockReturnValueOnce(new Promise((resolve) => (answer = resolve)));
      await userEvent.clear(field("Span 1 to"));
      await userEvent.type(field("Span 1 to"), "08:00{Enter}");
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
      act(() => answer({ spans: [rolled], totalMs: rolled.end - rolled.start }));
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [rolled] }), "cancel"));
    });

    it("saves the old time when the typed text is not a time", async () => {
      await openWithProject();
      await userEvent.clear(field("Span 1 to"));
      await userEvent.type(field("Span 1 to"), "later{Enter}");
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [{ start: START, end: END }] }), "cancel"));
    });

    it("saves the newest typed time when an older reading answers after it", async () => {
      await openWithProject();
      await userEvent.clear(field("Span 1 to"));
      let older: (ms: number | null) => void = () => {};
      mocked.calendarParseTime.mockReturnValueOnce(new Promise((resolve) => (older = resolve)));
      await userEvent.type(field("Span 1 to"), "1");
      mocked.calendarParseTime.mockResolvedValue(at("11:15"));
      await userEvent.type(field("Span 1 to"), "2");
      act(() => older(at("10:45")));
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [{ start: START, end: at("11:15") }] }), "cancel"));
    });
  });

  describe("duration", () => {
    it("asks Rust for the typed total and shows the end it moves to", async () => {
      renderCreate();
      await editorForm("New recording");
      fireEvent.change(field("Hours"), { target: { value: "2" } });
      expect(mocked.recordingDraftTotal).toHaveBeenCalledWith([{ start: START, end: END }], 150 * MINUTE_MS);
      await waitFor(() => expect(field("Span 1 to")).toHaveValue("11:30"));
      expect(field("Span 1 from")).toHaveValue("09:00");
    });

    it("moves the end of the last span only when the recording has several", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.click(button("Add span"));
      await screen.findByLabelText("Span 2 from");
      fireEvent.change(field("Minutes"), { target: { value: "45" } });
      await waitFor(() => expect(field("Span 2 to")).toHaveValue("11:45"));
      expect(field("Span 1 to")).toHaveValue("10:30");
    });

    it("never asks for a total of zero", async () => {
      renderCreate();
      await editorForm("New recording");
      fireEvent.change(field("Hours"), { target: { value: "0" } });
      await waitFor(() => expect(field("Span 1 to")).toHaveValue("09:30"));
      fireEvent.change(field("Minutes"), { target: { value: "0" } });
      expect(mocked.recordingDraftTotal).not.toHaveBeenCalledWith(expect.anything(), 0);
      expect(field("Minutes")).toHaveValue("30");
    });
  });

  describe("spans", () => {
    it("adds the row Rust places after the last span with Add span", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.click(button("Add span"));
      expect(mocked.recordingDraftAdd).toHaveBeenCalledWith([{ start: START, end: END }]);
      expect(await screen.findByLabelText("Span 2 from")).toHaveValue("10:30");
      expect(field("Span 2 to")).toHaveValue("11:00");
      await userEvent.click(button("Save"));
      await waitFor(() =>
        expect(mocked.recordingCreate).toHaveBeenCalledWith(
          expect.objectContaining({
            spans: [
              { start: START, end: END },
              { start: END, end: END + 30 * MINUTE_MS },
            ],
          }),
          "cancel",
        ),
      );
    });

    it("removes a row with its ✕ and renumbers the rest", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.click(button("Add span"));
      await screen.findByLabelText("Span 2 from");
      await userEvent.click(button("Remove span 09:00–10:30"));
      expect(screen.queryByLabelText("Span 2 from")).not.toBeInTheDocument();
      expect(field("Span 1 from")).toHaveValue("10:30");
    });

    it("has no ✕ while there is a single span", async () => {
      renderCreate();
      await editorForm("New recording");
      expect(screen.queryByRole("button", { name: /^Remove span/ })).not.toBeInTheDocument();
    });
  });

  describe("overlapping another recording", () => {
    const inTheWay = recording({ id: "r9", projectId: "p-site", note: "fix login", start: at("09:15"), end: at("09:45") });

    async function saveIntoOverlap(details = [inTheWay]) {
      mocked.recordingCreate.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps a recording", code: null, details });
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.click(button("Save"));
      return within(await screen.findByRole("alert"));
    }

    it("lists the recordings in the way inside the editor, not in a dialog", async () => {
      const notice = await saveIntoOverlap();
      expect(notice.getByText("This overlaps a recording")).toBeInTheDocument();
      expect(notice.getByRole("listitem")).toHaveTextContent("09:15–09:45 Website · fix login");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("lists every span of a recording in the way", async () => {
      const sittings = recording({
        ...inTheWay,
        spans: [
          { id: "r9-s1", start: at("09:15"), end: at("09:45") },
          { id: "r9-s2", start: at("16:05"), end: at("20:15") },
        ],
      });
      const notice = await saveIntoOverlap([sittings]);
      expect(notice.getByRole("listitem")).toHaveTextContent("09:15–09:45, 16:05–20:15 Website · fix login");
    });

    it("saves again with overlap replace when Replace is chosen, then closes the editor", async () => {
      const notice = await saveIntoOverlap();
      await userEvent.click(notice.getByRole("button", { name: "Replace overlapped part" }));
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenLastCalledWith({ projectId: "p-site", spans: [{ start: START, end: END }], note: "", tagId: null }, "replace"));
      await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
    });

    it("saves nothing more when Back is chosen and keeps the editor open", async () => {
      const notice = await saveIntoOverlap();
      await userEvent.click(notice.getByRole("button", { name: "Back" }));
      await waitFor(() => expect(screen.queryByText("This overlaps a recording")).not.toBeInTheDocument());
      expect(mocked.recordingCreate).not.toHaveBeenCalledWith(expect.anything(), "replace");
      expect(screen.getByRole("form", { name: "New recording" })).toBeInTheDocument();
    });
  });

  describe("editing", () => {
    beforeEach(() => mocked.recordingsList.mockResolvedValue([standupRow]));

    it("loads the whole recording of the block and shows its fields", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      expect(mocked.recordingsList).toHaveBeenCalledWith(standupBlock.start, standupBlock.end);
      expect(field("Span 1 from")).toHaveValue("11:00");
      expect(field("Span 1 to")).toHaveValue("11:45");
      expect(field("Note")).toHaveValue("standup");
      expect(projectField()).toHaveValue("Acme / Website");
    });

    it("shows the tag of the recording", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      expect(tagField()).toHaveValue("research");
    });

    it("does not prefill a project from the proposal", async () => {
      mocked.recordingProposal.mockResolvedValue({ projectId: "p-intra" });
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      await waitFor(() => expect(mocked.recordingProposal).toHaveBeenCalled());
      expect(projectField()).toHaveValue("Acme / Website");
    });

    it("sends tagId null when the tag is cleared with Backspace", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      await userEvent.click(tagField());
      await userEvent.keyboard("{Backspace}");
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", expect.objectContaining({ tagId: null }), "cancel"));
    });

    it("sends tagId null when No tag is picked", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      await userEvent.click(tagField());
      await userEvent.keyboard("{ArrowDown}");
      await userEvent.click(screen.getByRole("option", { name: "No tag" }));
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", expect.objectContaining({ tagId: null }), "cancel"));
    });

    it("shows the times of a recording that was cut to the day", async () => {
      const longer = recording({ ...standupRow, start: at("22:00", "2026-03-09"), end: at("01:00") });
      mocked.recordingsList.mockResolvedValue([longer]);
      renderEditor({ mode: "edit", id: "r1" }, { dayView: dayView({ recordings: [{ ...standupBlock, start: at("00:00"), end: at("01:00"), clippedStart: true }] }) });
      await editorForm("Edit recording");
      expect(field("Span 1 from")).toHaveValue("22:00");
      expect(field("Span 1 to")).toHaveValue("01:00");
      expect(field("Hours")).toHaveValue("3");
    });

    it("finds the recording by the span the opener names when the day does not show it", async () => {
      renderEditor({ mode: "edit", id: "r1", at: { start: standupBlock.start, end: standupBlock.end } }, { dayView: dayView() });
      await editorForm("Edit recording");
      expect(mocked.recordingsList).toHaveBeenCalledWith(standupBlock.start, standupBlock.end);
      expect(field("Span 1 from")).toHaveValue("11:00");
    });

    it("saves the changes to the recording with overlap cancel", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
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
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
    });

    it("deletes the recording with Delete and closes", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      await userEvent.click(button("Delete"));
      await waitFor(() => expect(mocked.recordingDelete).toHaveBeenCalledWith("r1"));
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
      expect(await screen.findByText("Recording deleted")).toBeInTheDocument();
    });

    it("deletes the recording with ⌘Backspace", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      await userEvent.keyboard("{Meta>}{Backspace}{/Meta}");
      await waitFor(() => expect(mocked.recordingDelete).toHaveBeenCalledWith("r1"));
    });

    describe("with several spans", () => {
      const morning = { id: "r1-s1", start: at("09:00"), end: at("09:30") };
      const noon = { id: "r1-s2", start: at("11:00"), end: at("11:45") };
      const evening = { id: "r1-s3", start: at("16:00"), end: at("17:00") };
      const sittings = recording({ ...standupRow, spans: [morning, noon, evening] });

      beforeEach(() => mocked.recordingsList.mockResolvedValue([sittings]));

      it("shows every span as a row", async () => {
        renderEditor({ mode: "edit", id: "r1" }, { dayView: dayView({ recordings: [{ ...standupBlock, spanId: noon.id, spanCount: 3 }] }) });
        await editorForm("Edit recording");
        expect([1, 2, 3].map((n) => (field(`Span ${n} from`) as HTMLInputElement).value)).toEqual(["09:00", "11:00", "16:00"]);
        expect([1, 2, 3].map((n) => (field(`Span ${n} to`) as HTMLInputElement).value)).toEqual(["09:30", "11:45", "17:00"]);
        expect(screen.queryByText(/^Also /)).not.toBeInTheDocument();
      });

      it("saves all spans with only the edited one changed", async () => {
        renderEditor({ mode: "edit", id: "r1" });
        await editorForm("Edit recording");
        await userEvent.clear(field("Span 2 to"));
        await userEvent.type(field("Span 2 to"), "12:00");
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

      it("drops a span removed with its ✕ from the save", async () => {
        renderEditor({ mode: "edit", id: "r1" });
        await editorForm("Edit recording");
        await userEvent.click(button("Remove span 09:00–09:30"));
        await userEvent.click(button("Save"));
        await waitFor(() =>
          expect(mocked.recordingUpdate).toHaveBeenCalledWith(
            "r1",
            expect.objectContaining({
              spans: [
                { start: noon.start, end: noon.end },
                { start: evening.start, end: evening.end },
              ],
            }),
            "cancel",
          ),
        );
      });

      it("shows no ✕ for a recording with one span", async () => {
        mocked.recordingsList.mockResolvedValue([standupRow]);
        renderEditor({ mode: "edit", id: "r1" });
        await editorForm("Edit recording");
        expect(screen.queryByRole("button", { name: /^Remove span/ })).not.toBeInTheDocument();
      });
    });

    describe("while the timer runs on it", () => {
      const morning = { id: "r1-s1", start: at("09:00"), end: at("09:30") };
      const running = recording({ ...standupRow, spans: [morning, { id: "r1-s2", start: at("11:00"), end: null }] });

      beforeEach(() => mocked.recordingsList.mockResolvedValue([running]));

      it("shows the spans without fields to edit them and without Add span", async () => {
        renderEditor({ mode: "edit", id: "r1" });
        await editorForm("Edit recording");
        expect(screen.queryByLabelText("Span 1 from")).not.toBeInTheDocument();
        expect(screen.getByText("09:00")).toBeInTheDocument();
        expect(screen.getByText("11:00")).toBeInTheDocument();
        expect(screen.getByText("running")).toBeInTheDocument();
        expect(screen.getAllByText("From")).toHaveLength(2);
        expect(screen.getByText("to")).toBeInTheDocument();
        expect(screen.queryByRole("button", { name: "Add span" })).not.toBeInTheDocument();
        expect(field("Hours")).toHaveAttribute("readonly");
      });

      it("saves the fields without spans", async () => {
        renderEditor({ mode: "edit", id: "r1" });
        await editorForm("Edit recording");
        await userEvent.type(field("Note"), "!");
        await userEvent.click(button("Save"));
        await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", { projectId: "p-site", note: "standup!", tagId: "tag-research" }, "cancel"));
      });
    });

    it("shows nothing and an error when the recording cannot be loaded", async () => {
      mocked.recordingsList.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      renderEditor({ mode: "edit", id: "r1" });
      expect(await screen.findByText("Could not open the recording: Database locked")).toBeInTheDocument();
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
    });
  });
});
