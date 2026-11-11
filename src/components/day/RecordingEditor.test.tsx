import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type DayView, type Project, type Recording, type Settings } from "../../api";
import type { Editor } from "../../state/slices/editor";
import { resetStore, useStore, type Store } from "../../state/store";
import { at, DAY, dayView, recordingBlock } from "../../test/dayFixtures";
import { client, projectItem } from "../../test/projectFixtures";
import { dayKey, renderWithClient, seedProjects, seedQuery, SETTINGS_KEY } from "../../test/queries";
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

/** Store state, plus the query data the editor reads: the recent projects, the settings and the shown day's view. */
type EditorSetup = Partial<Store> & { recent?: Project[]; settings?: Settings; dayView?: DayView };

function renderEditor(editor: Editor, { recent = [], settings: shownSettings = settings(), dayView: shownDay = dayView({ recordings: [standupBlock] }), ...patch }: EditorSetup = {}) {
  resetStore({ day: shownDay.day, ...patch });
  seedQuery(SETTINGS_KEY, shownSettings);
  seedQuery(dayKey(shownDay.day), shownDay);
  seedProjects({ projects: [website, intranet], clients: [acme], recent });
  act(() => useStore.getState().openEditor(editor));
  renderWithClient(
    <>
      <RecordingEditor />
      <Dialogs />
      <Toasts />
    </>,
  );
}

const renderCreate = (patch: EditorSetup = {}) => renderEditor({ mode: "create", start: START, end: END }, patch);
const editorForm = (name: "New recording" | "Edit recording" | "Edit suggestion") => screen.findByRole("form", { name });
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
      expect(field("Time 1 from")).toHaveValue("09:00");
      expect(field("Time 1 to")).toHaveValue("10:30");
      expect(field("Hours")).toHaveValue("1");
      expect(field("Minutes")).toHaveValue("30");
    });

    it("shows no text labels beside the fields and joins the span times with a dash", async () => {
      renderCreate();
      const form = within(await editorForm("New recording"));
      for (const label of ["Project", "Tag", "Duration", "From", "to"]) expect(form.queryByText(label)).not.toBeInTheDocument();
      expect(form.getByRole("group", { name: "Times" })).toHaveTextContent("–");
    });

    it("opens the project list when the empty Project field is tabbed into", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.tab();
      expect(projectField()).toHaveFocus();
      expect(screen.getByRole("listbox", { name: "Projects" })).toBeInTheDocument();
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
      expect(await screen.findByText("Recorded 1h 30m for Website")).toBeInTheDocument();
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
      await userEvent.click(field("Time 1 from"));
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

    it("offers the proposed project first, marked Suggested, without filling it in", async () => {
      mocked.recordingProposal.mockResolvedValue({ projectId: "p-site" });
      renderCreate({ recent: [intranet.project] });
      await editorForm("New recording");
      const list = within(await screen.findByRole("list", { name: "Recent projects" }));
      expect(await list.findByText("Suggested")).toBeInTheDocument();
      const rows = list.getAllByRole("button");
      expect(rows.map((row) => row.getAttribute("aria-label"))).toEqual(["Acme / Website", "Intranet"]);
      expect(rows[0]).toHaveTextContent("Suggested");
      expect(rows[1]).not.toHaveTextContent("Suggested");
      expect(projectField()).toHaveValue("");
    });

    it("sets the project when the Suggested row is clicked", async () => {
      mocked.recordingProposal.mockResolvedValue({ projectId: "p-site" });
      renderCreate();
      await editorForm("New recording");
      const list = within(await screen.findByRole("list", { name: "Recent projects" }));
      await list.findByText("Suggested");
      await userEvent.click(list.getByRole("button", { name: "Acme / Website" }));
      expect(projectField()).toHaveValue("Website");
      expect(tagField()).toBeInTheDocument();
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
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), "11:15");
      await waitFor(() => expect(mocked.recordingProposal).toHaveBeenLastCalledWith([{ start: START, end: at("11:15") }]));
    });

    it("has no Activity button", async () => {
      renderCreate();
      await editorForm("New recording");
      await waitFor(() => expect(mocked.recordingProposal).toHaveBeenCalled());
      expect(screen.queryByRole("button", { name: /^Activity/ })).not.toBeInTheDocument();
    });
  });

  describe("autofill", () => {
    const earlier = { projectId: "p-site", tagId: "tag-research" };
    const autofillButton = () => screen.queryByRole("button", { name: "Autofill" });

    beforeEach(() => mocked.recordingAutofill.mockResolvedValue(null));
    afterEach(() => mocked.recordingAutofill.mockResolvedValue(null));

    it("asks Rust for the project and tag of the note once typing settles", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.type(field("Note"), "fix login");
      await waitFor(() => expect(mocked.recordingAutofill).toHaveBeenCalledWith("fix login"));
    });

    it("fills the project and tag of the earlier recording when Autofill is clicked, then hides the button", async () => {
      mocked.recordingAutofill.mockResolvedValue(earlier);
      renderCreate();
      await editorForm("New recording");
      await userEvent.type(field("Note"), "fix login");
      await userEvent.click(await screen.findByRole("button", { name: "Autofill" }));
      expect(projectField()).toHaveValue("Website");
      expect(tagField()).toHaveValue("research");
      expect(autofillButton()).not.toBeInTheDocument();
    });

    it("offers no Autofill when Rust knows no earlier recording with the note", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.type(field("Note"), "fix login");
      await waitFor(() => expect(mocked.recordingAutofill).toHaveBeenCalledWith("fix login"));
      await act(async () => {});
      expect(autofillButton()).not.toBeInTheDocument();
    });

    it("offers no Autofill when the draft already has that project and tag", async () => {
      mocked.recordingAutofill.mockResolvedValue(earlier);
      mocked.recordingsList.mockResolvedValue([standupRow]);
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      await waitFor(() => expect(mocked.recordingAutofill).toHaveBeenCalledWith("standup"));
      await act(async () => {});
      expect(autofillButton()).not.toBeInTheDocument();
    });
  });

  describe("tag field", () => {
    it("is absent for a new recording without a project", async () => {
      renderCreate();
      await editorForm("New recording");
      await waitFor(() => expect(mocked.recordingProposal).toHaveBeenCalled());
      expect(screen.queryByRole("combobox", { name: "Tag" })).not.toBeInTheDocument();
    });

    it("appears once a project is picked", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      expect(tagField()).toBeInTheDocument();
    });

    it("warns with ⚠️ while no tag is chosen, and still saves the recording untagged", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      expect(tagField().parentElement).toHaveTextContent("⚠️");
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith({ projectId: "p-site", spans: [{ start: START, end: END }], note: "", tagId: null }, "cancel"));
    });

    it("drops the ⚠️ once a tag is chosen", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.type(tagField(), "res{Enter}");
      expect(tagField().parentElement).not.toHaveTextContent("⚠️");
    });

    it("is shown when a recording with a project is edited", async () => {
      mocked.recordingsList.mockResolvedValue([standupRow]);
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      expect(tagField()).toBeInTheDocument();
    });
  });

  describe("recent projects", () => {
    const withRecent: EditorSetup = { recent: [website.project, intranet.project] };
    const recentList = () => screen.queryByRole("list", { name: "Recent projects" });

    it("lists the recent projects below the Project field while none is picked", async () => {
      renderCreate(withRecent);
      await editorForm("New recording");
      const list = within(screen.getByRole("list", { name: "Recent projects" }));
      expect(list.getByRole("button", { name: "Acme / Website" })).toBeInTheDocument();
      expect(list.getByRole("button", { name: "Intranet" })).toBeInTheDocument();
    });

    it("picks the clicked project, then hides the list and shows the Tag field", async () => {
      renderCreate(withRecent);
      await editorForm("New recording");
      await userEvent.click(within(screen.getByRole("list", { name: "Recent projects" })).getByRole("button", { name: "Acme / Website" }));
      expect(projectField()).toHaveValue("Website");
      expect(recentList()).not.toBeInTheDocument();
      expect(tagField()).toBeInTheDocument();
    });

    it("is not shown when the recording already has a project", async () => {
      mocked.recordingsList.mockResolvedValue([standupRow]);
      renderEditor({ mode: "edit", id: "r1" }, withRecent);
      await editorForm("Edit recording");
      expect(recentList()).not.toBeInTheDocument();
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

    it("closes the editor with a second Escape in a picker, after the first closed its list", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.click(tagField());
      expect(screen.getByRole("listbox", { name: "Tags" })).toBeInTheDocument();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
      expect(screen.getByRole("form", { name: "New recording" })).toBeInTheDocument();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
    });
  });

  describe("times", () => {
    it("commits a typed time on blur and updates the length", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), "11:15");
      await userEvent.tab();
      expect(field("Time 1 to")).toHaveValue("11:15");
      await waitFor(() => expect(field("Hours")).toHaveValue("2"));
      expect(field("Minutes")).toHaveValue("15");
    });

    it("asks Rust about the spans once a valid time is typed", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), "11:15");
      await waitFor(() => expect(mocked.recordingDraftSpans).toHaveBeenLastCalledWith([{ start: START, end: at("11:15") }]));
    });

    it("counts a valid typed time at once, so Enter saves it without leaving the field", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), "11:15{Enter}");
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [{ start: START, end: at("11:15") }] }), "cancel"));
    });

    it("updates the length while a valid time is typed, before the field is left", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), "11:15");
      await waitFor(() => expect(field("Hours")).toHaveValue("2"));
      expect(field("Minutes")).toHaveValue("15");
    });

    it("accepts a time typed without a colon", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.clear(field("Time 1 from"));
      await userEvent.type(field("Time 1 from"), "0845");
      await userEvent.tab();
      expect(field("Time 1 from")).toHaveValue("08:45");
    });

    it("restores the previous time when the text is not a time", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), "later");
      await userEvent.tab();
      expect(field("Time 1 to")).toHaveValue("10:30");
    });

    it("steps a time by the snap interval with the arrow keys", async () => {
      renderCreate({ settings: settings({ timeline: { ...settings().timeline, snapMinutes: 15 } }) });
      await editorForm("New recording");
      await userEvent.type(field("Time 1 to"), "{ArrowUp}");
      expect(field("Time 1 to")).toHaveValue("10:45");
      await userEvent.type(field("Time 1 from"), "{ArrowDown}{ArrowDown}");
      expect(field("Time 1 from")).toHaveValue("08:30");
    });

    it("sends an end before the start as typed and shows the next-day end Rust rolls it to", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      const rolled = { start: START, end: at("08:00", "2026-03-11") };
      mocked.recordingDraftSpans.mockResolvedValueOnce({ spans: [rolled], totalMs: rolled.end - rolled.start });
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), "08:00");
      await waitFor(() => expect(mocked.recordingDraftSpans).toHaveBeenLastCalledWith([{ start: START, end: at("08:00") }]));
      await waitFor(() => expect(field("Hours")).toHaveValue("23"));
      expect(field("Minutes")).toHaveValue("0");
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [rolled] }), "cancel"));
    });
  });

  describe("a new recording's times", () => {
    it("write nothing until Save sends them", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), "11:15");
      await waitFor(() => expect(field("Hours")).toHaveValue("2"));
      expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [{ start: START, end: at("11:15") }] }), "cancel"));
    });

    it("stay in a suggestion's draft until Approve sends them", async () => {
      renderEditor({ mode: "create", start: START, end: END, projectId: "p-site", suggestion: { id: "s1", by: "Planner" } });
      await editorForm("Edit suggestion");
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), "11:15");
      await waitFor(() => expect(field("Hours")).toHaveValue("2"));
      expect(mocked.suggestionApproveEdited).not.toHaveBeenCalled();
      expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      await userEvent.click(button("Approve"));
      await waitFor(() =>
        expect(mocked.suggestionApproveEdited).toHaveBeenCalledWith(DAY, "s1", { projectId: "p-site", spans: [{ start: START, end: at("11:15") }], note: "", tagId: null }, "cancel"),
      );
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
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), `11:15${keys}`);
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
      await userEvent.clear(field("Time 1 from"));
      await userEvent.type(field("Time 1 from"), "1{Enter}");
      expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      act(() => readings.forEach((read) => read(at("10:45"))));
      await waitFor(() =>
        expect(mocked.recordingUpdate).toHaveBeenCalledWith(
          "r1",
          { projectId: "p-site", note: "standup", tagId: "tag-research", spans: [{ start: at("10:45"), end: standupBlock.end }] },
          "cancel",
        ),
      );
    });

    it("waits for Rust's answer about the spans and saves what it answered", async () => {
      await openWithProject();
      const rolled = { start: START, end: at("08:00", "2026-03-11") };
      let answer: (draft: { spans: { start: number; end: number }[]; totalMs: number }) => void = () => {};
      mocked.recordingDraftSpans.mockReturnValueOnce(new Promise((resolve) => (answer = resolve)));
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), "08:00{Enter}");
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
      act(() => answer({ spans: [rolled], totalMs: rolled.end - rolled.start }));
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [rolled] }), "cancel"));
    });

    it("saves the old time when the typed text is not a time", async () => {
      await openWithProject();
      await userEvent.clear(field("Time 1 to"));
      await userEvent.type(field("Time 1 to"), "later{Enter}");
      await waitFor(() => expect(mocked.recordingCreate).toHaveBeenCalledWith(expect.objectContaining({ spans: [{ start: START, end: END }] }), "cancel"));
    });

    it("saves the newest typed time when an older reading answers after it", async () => {
      await openWithProject();
      await userEvent.clear(field("Time 1 to"));
      let older: (ms: number | null) => void = () => {};
      mocked.calendarParseTime.mockReturnValueOnce(new Promise((resolve) => (older = resolve)));
      await userEvent.type(field("Time 1 to"), "1");
      mocked.calendarParseTime.mockResolvedValue(at("11:15"));
      await userEvent.type(field("Time 1 to"), "2");
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
      await waitFor(() => expect(field("Time 1 to")).toHaveValue("11:30"));
      expect(field("Time 1 from")).toHaveValue("09:00");
    });

    it("moves the end of the last span only when the recording has several", async () => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.click(button("Add time"));
      await screen.findByLabelText("Time 2 from");
      fireEvent.change(field("Minutes"), { target: { value: "45" } });
      await waitFor(() => expect(field("Time 2 to")).toHaveValue("11:45"));
      expect(field("Time 1 to")).toHaveValue("10:30");
    });

    it("never asks for a total of zero", async () => {
      renderCreate();
      await editorForm("New recording");
      fireEvent.change(field("Hours"), { target: { value: "0" } });
      await waitFor(() => expect(field("Time 1 to")).toHaveValue("09:30"));
      fireEvent.change(field("Minutes"), { target: { value: "0" } });
      expect(mocked.recordingDraftTotal).not.toHaveBeenCalledWith(expect.anything(), 0);
      expect(field("Minutes")).toHaveValue("30");
    });
  });

  describe("logged time", () => {
    const quick = () => within(screen.getByRole("group", { name: "Set duration" }));

    it("labels the band and offers 15m, 30m and 1h", async () => {
      renderCreate();
      await editorForm("New recording");
      expect(screen.getByText("Logged")).toBeInTheDocument();
      expect(quick().getAllByRole("button").map((b) => b.textContent)).toEqual(["15m", "30m", "1h"]);
    });

    it.each([
      ["15m", 15, "09:15", "0", "15"],
      ["30m", 30, "09:30", "0", "30"],
      ["1h", 60, "10:00", "1", "0"],
    ])("sets the length with %s and shows the end Rust moves to", async (label, minutes, end, hours, rest) => {
      renderCreate();
      await editorForm("New recording");
      await userEvent.click(quick().getByRole("button", { name: label }));
      expect(mocked.recordingDraftTotal).toHaveBeenCalledWith([{ start: START, end: END }], minutes * MINUTE_MS);
      await waitFor(() => expect(field("Time 1 to")).toHaveValue(end));
      expect(field("Hours")).toHaveValue(hours);
      expect(field("Minutes")).toHaveValue(rest);
    });

    it("presses only the button that matches the current length", async () => {
      renderCreate();
      await editorForm("New recording");
      for (const label of ["15m", "30m", "1h"]) expect(quick().getByRole("button", { name: label })).toHaveAttribute("aria-pressed", "false");
      await userEvent.click(quick().getByRole("button", { name: "30m" }));
      await waitFor(() => expect(quick().getByRole("button", { name: "30m" })).toHaveAttribute("aria-pressed", "true"));
      expect(quick().getByRole("button", { name: "15m" })).toHaveAttribute("aria-pressed", "false");
      expect(quick().getByRole("button", { name: "1h" })).toHaveAttribute("aria-pressed", "false");
    });

    it("is not offered while the timer runs on the recording", async () => {
      const running = recording({ ...standupRow, spans: [{ id: "r1-s1", start: at("11:00"), end: null }] });
      mocked.recordingsList.mockResolvedValue([running]);
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      expect(screen.getByText("Logged")).toBeInTheDocument();
      expect(screen.queryByRole("group", { name: "Set duration" })).not.toBeInTheDocument();
    });
  });

  describe("spans", () => {
    it("adds the row Rust places after the last span with Add time", async () => {
      renderCreate();
      await editorForm("New recording");
      await chooseWebsite();
      await userEvent.click(button("Add time"));
      expect(mocked.recordingDraftAdd).toHaveBeenCalledWith([{ start: START, end: END }]);
      expect(await screen.findByLabelText("Time 2 from")).toHaveValue("10:30");
      expect(field("Time 2 to")).toHaveValue("11:00");
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
      await userEvent.click(button("Add time"));
      await screen.findByLabelText("Time 2 from");
      await userEvent.click(button("Remove time 09:00–10:30"));
      expect(screen.queryByLabelText("Time 2 from")).not.toBeInTheDocument();
      expect(field("Time 1 from")).toHaveValue("10:30");
    });

    it("has no ✕ while there is a single span", async () => {
      renderCreate();
      await editorForm("New recording");
      expect(screen.queryByRole("button", { name: /^Remove time/ })).not.toBeInTheDocument();
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
      expect(field("Time 1 from")).toHaveValue("11:00");
      expect(field("Time 1 to")).toHaveValue("11:45");
      expect(field("Note")).toHaveValue("standup");
      expect(projectField()).toHaveValue("Website");
      expect(screen.getByText("Acme")).toBeInTheDocument();
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
      expect(projectField()).toHaveValue("Website");
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
      expect(field("Time 1 from")).toHaveValue("22:00");
      expect(field("Time 1 to")).toHaveValue("01:00");
      expect(field("Hours")).toHaveValue("3");
    });

    it("finds the recording by the span the opener names when the day does not show it", async () => {
      renderEditor({ mode: "edit", id: "r1", at: { start: standupBlock.start, end: standupBlock.end } }, { dayView: dayView() });
      await editorForm("Edit recording");
      expect(mocked.recordingsList).toHaveBeenCalledWith(standupBlock.start, standupBlock.end);
      expect(field("Time 1 from")).toHaveValue("11:00");
    });

    it("saves the project, note and tag only, with overlap cancel", async () => {
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      await userEvent.type(field("Note"), " notes");
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", { projectId: "p-site", note: "standup notes", tagId: "tag-research" }, "cancel"));
      expect(await screen.findByText("Recording changed")).toBeInTheDocument();
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
    });

    describe("times", () => {
      const quick = () => within(screen.getByRole("group", { name: "Set duration" }));
      const inTheWay = recording({ id: "r9", projectId: "p-site", note: "fix login", start: at("11:20"), end: at("11:50") });

      async function openEdited() {
        renderEditor({ mode: "edit", id: "r1" });
        await editorForm("Edit recording");
      }

      async function typeStart(time: string) {
        await userEvent.clear(field("Time 1 from"));
        await userEvent.type(field("Time 1 from"), time);
      }

      const typedSpan = [{ start: at("11:10"), end: standupBlock.end }];

      async function expectNothingWritten() {
        await act(async () => {});
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
        expect(screen.queryByText("Recording changed")).not.toBeInTheDocument();
      }

      it("keeps a typed time in the draft: Rust's answer shows, nothing is written, the editor stays open", async () => {
        await openEdited();
        await typeStart("11:10");
        await waitFor(() => expect(mocked.recordingDraftSpans).toHaveBeenLastCalledWith(typedSpan));
        expect(field("Time 1 from")).toHaveValue("11:10");
        await expectNothingWritten();
        expect(screen.getByRole("form", { name: "Edit recording" })).toBeInTheDocument();
      });

      it("keeps the spans Rust added with Add time in the draft", async () => {
        await openEdited();
        await userEvent.click(button("Add time"));
        expect(await screen.findByLabelText("Time 2 from")).toHaveValue("11:45");
        expect(field("Time 2 to")).toHaveValue("12:15");
        await expectNothingWritten();
      });

      it.each([
        ["15m", "11:15"],
        ["30m", "11:30"],
        ["1h", "12:00"],
      ])("keeps the length chosen with %s in the draft", async (label, end) => {
        await openEdited();
        await userEvent.click(quick().getByRole("button", { name: label }));
        await waitFor(() => expect(field("Time 1 to")).toHaveValue(end));
        await expectNothingWritten();
      });

      it("keeps a typed length in the draft", async () => {
        await openEdited();
        fireEvent.change(field("Hours"), { target: { value: "2" } });
        await waitFor(() => expect(field("Time 1 to")).toHaveValue("13:45"));
        await expectNothingWritten();
      });

      it("keeps a removed span in the draft", async () => {
        await openEdited();
        await userEvent.click(button("Add time"));
        await screen.findByLabelText("Time 2 from");
        await userEvent.click(button("Remove time 11:45–12:15"));
        await waitFor(() => expect(screen.queryByLabelText("Time 2 from")).not.toBeInTheDocument());
        await expectNothingWritten();
      });

      it("writes the changed times with project, note and tag on Save, then closes with a Recording changed toast", async () => {
        await openEdited();
        await typeStart("11:10");
        await waitFor(() => expect(field("Time 1 from")).toHaveValue("11:10"));
        await userEvent.type(field("Note"), " notes");
        await userEvent.click(button("Save"));
        await waitFor(() =>
          expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", { projectId: "p-site", note: "standup notes", tagId: "tag-research", spans: typedSpan }, "cancel"),
        );
        expect(await screen.findByText("Recording changed")).toBeInTheDocument();
        expect(button("Undo")).toBeInTheDocument();
        await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
      });

      it("writes the spans Rust added on Save", async () => {
        await openEdited();
        await userEvent.click(button("Add time"));
        await screen.findByLabelText("Time 2 from");
        await userEvent.click(button("Save"));
        await waitFor(() =>
          expect(mocked.recordingUpdate).toHaveBeenCalledWith(
            "r1",
            {
              projectId: "p-site",
              note: "standup",
              tagId: "tag-research",
              spans: [
                { start: standupBlock.start, end: standupBlock.end },
                { start: standupBlock.end, end: standupBlock.end + 30 * MINUTE_MS },
              ],
            },
            "cancel",
          ),
        );
      });

      it("waits for Rust's answer about a typed time before Save writes, then closes", async () => {
        const answer: { settle?: (draft: { spans: { start: number; end: number }[]; totalMs: number }) => void } = {};
        const answered = mocked.recordingDraftSpans.getMockImplementation();
        mocked.recordingDraftSpans.mockImplementation((spans) =>
          spans[0].start === at("10:45") ? new Promise((resolve) => (answer.settle = resolve)) : (answered?.(spans) ?? Promise.reject(new Error("no default"))),
        );
        try {
          await openEdited();
          await typeStart("10:45");
          await waitFor(() => expect(answer.settle).toBeDefined());
          await userEvent.click(button("Save"));
          expect(screen.getByRole("form", { name: "Edit recording" })).toBeInTheDocument();
          expect(mocked.recordingUpdate).not.toHaveBeenCalled();
          const rolled = [{ start: at("10:45"), end: standupBlock.end }];
          act(() => answer.settle?.({ spans: rolled, totalMs: standupBlock.end - at("10:45") }));
          await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", { projectId: "p-site", note: "standup", tagId: "tag-research", spans: rolled }, "cancel"));
          await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
        } finally {
          if (answered) mocked.recordingDraftSpans.mockImplementation(answered);
        }
      });

      it("asks before leaving the view after a time change, and stays on Keep editing", async () => {
        await openEdited();
        await typeStart("11:10");
        await waitFor(() => expect(field("Time 1 from")).toHaveValue("11:10"));
        act(() => useStore.getState().setView("week"));
        expect(screen.getByRole("heading", { name: "Discard changes?" })).toBeInTheDocument();
        expect(useStore.getState().view).toBe("day");
        await userEvent.click(button("Keep editing"));
        expect(field("Time 1 from")).toHaveValue("11:10");
      });

      it("asks before leaving the view with a changed note", async () => {
        await openEdited();
        await userEvent.type(field("Note"), " notes");
        act(() => useStore.getState().setView("week"));
        expect(screen.getByRole("heading", { name: "Discard changes?" })).toBeInTheDocument();
        expect(useStore.getState().view).toBe("day");
      });

      it("closes with Cancel after a time change, writes nothing and does not ask", async () => {
        await openEdited();
        await typeStart("11:10");
        await waitFor(() => expect(field("Time 1 from")).toHaveValue("11:10"));
        await userEvent.click(button("Cancel"));
        expect(screen.queryByRole("form")).not.toBeInTheDocument();
        expect(screen.queryByRole("heading", { name: "Discard changes?" })).not.toBeInTheDocument();
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      });

      it("closes with Escape after a time change and writes nothing", async () => {
        await openEdited();
        await userEvent.click(button("Add time"));
        await screen.findByLabelText("Time 2 from");
        await userEvent.keyboard("{Escape}");
        expect(screen.queryByRole("form")).not.toBeInTheDocument();
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
      });

      it("stays open with the changed times and toasts when Save fails", async () => {
        mocked.recordingUpdate.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
        await openEdited();
        await typeStart("11:10");
        await waitFor(() => expect(field("Time 1 from")).toHaveValue("11:10"));
        await userEvent.click(button("Save"));
        expect(await screen.findByText("Could not change the recording: Disk full")).toBeInTheDocument();
        expect(screen.getByRole("form", { name: "Edit recording" })).toBeInTheDocument();
        expect(field("Time 1 from")).toHaveValue("11:10");
      });

      it("asks about an overlap on Save inside the editor and keeps the changed times on Back", async () => {
        mocked.recordingUpdate.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps a recording", code: null, details: [inTheWay] });
        await openEdited();
        await typeStart("11:10");
        await waitFor(() => expect(field("Time 1 from")).toHaveValue("11:10"));
        await userEvent.click(button("Save"));
        const notice = within(await screen.findByRole("alert"));
        expect(notice.getByRole("listitem")).toHaveTextContent("11:20–11:50 Website · fix login");
        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
        await userEvent.click(notice.getByRole("button", { name: "Back" }));
        await waitFor(() => expect(screen.queryByText("This overlaps a recording")).not.toBeInTheDocument());
        expect(screen.getByRole("form", { name: "Edit recording" })).toBeInTheDocument();
        expect(field("Time 1 from")).toHaveValue("11:10");
      });

      it("saves again with overlap replace when Replace is chosen, then closes", async () => {
        mocked.recordingUpdate.mockRejectedValueOnce({ kind: "overlap", message: "Overlaps a recording", code: null, details: [inTheWay] });
        await openEdited();
        await typeStart("11:10");
        await waitFor(() => expect(field("Time 1 from")).toHaveValue("11:10"));
        await userEvent.click(button("Save"));
        const notice = within(await screen.findByRole("alert"));
        await userEvent.click(notice.getByRole("button", { name: "Replace overlapped part" }));
        await waitFor(() =>
          expect(mocked.recordingUpdate).toHaveBeenLastCalledWith("r1", { projectId: "p-site", note: "standup", tagId: "tag-research", spans: typedSpan }, "replace"),
        );
        await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
      });

      it("takes the times Rust holds when the day refetched, keeping the typed note", async () => {
        await openEdited();
        await userEvent.type(field("Note"), " notes");
        mocked.recordingsList.mockResolvedValue([recording({ ...standupRow, spans: [{ id: "r1-s1", start: at("11:05"), end: at("11:50") }] })]);
        act(() => seedQuery(dayKey(DAY), dayView({ recordings: [{ ...standupBlock, start: at("11:05"), end: at("11:50") }] })));
        await waitFor(() => expect(field("Time 1 from")).toHaveValue("11:05"));
        expect(field("Time 1 to")).toHaveValue("11:50");
        expect(field("Note")).toHaveValue("standup notes");
      });

      it("leaves without asking after such a sync", async () => {
        await openEdited();
        mocked.recordingsList.mockResolvedValue([recording({ ...standupRow, spans: [{ id: "r1-s1", start: at("11:05"), end: at("11:50") }] })]);
        act(() => seedQuery(dayKey(DAY), dayView({ recordings: [{ ...standupBlock, start: at("11:05"), end: at("11:50") }] })));
        await waitFor(() => expect(field("Time 1 from")).toHaveValue("11:05"));
        act(() => useStore.getState().setView("week"));
        expect(screen.queryByRole("heading", { name: "Discard changes?" })).not.toBeInTheDocument();
        expect(useStore.getState().view).toBe("week");
      });
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
        expect([1, 2, 3].map((n) => (field(`Time ${n} from`) as HTMLInputElement).value)).toEqual(["09:00", "11:00", "16:00"]);
        expect([1, 2, 3].map((n) => (field(`Time ${n} to`) as HTMLInputElement).value)).toEqual(["09:30", "11:45", "17:00"]);
        expect(screen.queryByText(/^Also /)).not.toBeInTheDocument();
      });

      it("writes all spans on Save with only the edited one changed", async () => {
        renderEditor({ mode: "edit", id: "r1" });
        await editorForm("Edit recording");
        await userEvent.clear(field("Time 2 to"));
        await userEvent.type(field("Time 2 to"), "12:00");
        await waitFor(() => expect(field("Time 2 to")).toHaveValue("12:00"));
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
        await userEvent.click(button("Save"));
        await waitFor(() =>
          expect(mocked.recordingUpdate).toHaveBeenCalledWith(
            "r1",
            {
              projectId: "p-site",
              note: "standup",
              tagId: "tag-research",
              spans: [
                { start: morning.start, end: morning.end },
                { start: noon.start, end: at("12:00") },
                { start: evening.start, end: evening.end },
              ],
            },
            "cancel",
          ),
        );
      });

      it("writes the other spans on Save when a ✕ removed one", async () => {
        renderEditor({ mode: "edit", id: "r1" });
        await editorForm("Edit recording");
        await userEvent.click(button("Remove time 09:00–09:30"));
        await waitFor(() => expect(screen.queryByLabelText("Time 3 from")).not.toBeInTheDocument());
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
        await userEvent.click(button("Save"));
        await waitFor(() =>
          expect(mocked.recordingUpdate).toHaveBeenCalledWith(
            "r1",
            {
              projectId: "p-site",
              note: "standup",
              tagId: "tag-research",
              spans: [
                { start: noon.start, end: noon.end },
                { start: evening.start, end: evening.end },
              ],
            },
            "cancel",
          ),
        );
      });

      it("shows no ✕ for a recording with one span", async () => {
        mocked.recordingsList.mockResolvedValue([standupRow]);
        renderEditor({ mode: "edit", id: "r1" });
        await editorForm("Edit recording");
        expect(screen.queryByRole("button", { name: /^Remove time/ })).not.toBeInTheDocument();
      });
    });

    describe("while the timer runs on it", () => {
      const morning = { id: "r1-s1", start: at("09:00"), end: at("09:30") };
      const running = recording({ ...standupRow, spans: [morning, { id: "r1-s2", start: at("11:00"), end: null }] });

      const openSince = at("11:00");
      /** The recording as Rust saves it: closed spans and the timer's open span from `openStart`. */
      const savedRunning = (closed: { start: number; end: number }[], openStart: number) =>
        recording({ ...standupRow, spans: [...closed.map((s, i) => ({ id: `r1-s${i + 1}`, ...s })), { id: `r1-s${closed.length + 1}`, start: openStart, end: null }] });

      beforeEach(() => {
        mocked.recordingsList.mockResolvedValue([running]);
        // Rust keeps the times a patch leaves out, and the timer running.
        let closed = [{ start: morning.start, end: morning.end }];
        let openStart = openSince;
        mocked.recordingUpdate.mockImplementation(async (_id, patch) => {
          closed = patch.spans ?? closed;
          openStart = patch.openStart ?? openStart;
          return savedRunning(closed, openStart);
        });
      });

      afterEach(() => void mocked.recordingUpdate.mockReset());

      async function openRunning(setup: EditorSetup = {}) {
        renderEditor({ mode: "edit", id: "r1" }, setup);
        await editorForm("Edit recording");
      }

      it("shows the closed spans as rows and the open span last with its start as a field and a running end", async () => {
        await openRunning();
        expect(field("Time 1 from")).toHaveValue("09:00");
        expect(field("Time 1 to")).toHaveValue("09:30");
        expect(field("Time 2 from")).toHaveValue("11:00");
        expect(screen.queryByLabelText("Time 2 to")).not.toBeInTheDocument();
        expect(screen.getByText("– running")).toBeInTheDocument();
        expect(button("Add time")).toBeInTheDocument();
        expect(field("Hours")).toHaveAttribute("readonly");
      });

      it("shows the open span alone without a ✕", async () => {
        mocked.recordingsList.mockResolvedValue([recording({ ...standupRow, spans: [{ id: "r1-s1", start: openSince, end: null }] })]);
        await openRunning();
        expect(field("Time 1 from")).toHaveValue("11:00");
        expect(screen.getByText("– running")).toBeInTheDocument();
        expect(screen.queryByRole("button", { name: /^Remove time/ })).not.toBeInTheDocument();
      });

      it("offers Save and Stop Timer, and no Stop and save", async () => {
        await openRunning();
        expect(button("Save")).toBeInTheDocument();
        expect(button("Stop Timer")).toBeInTheDocument();
        expect(screen.queryByRole("button", { name: "Stop and save" })).not.toBeInTheDocument();
      });

      it("saves the fields without spans, closes the editor and leaves the timer running", async () => {
        await openRunning();
        await userEvent.type(field("Note"), "!");
        await userEvent.click(button("Save"));
        await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", { projectId: "p-site", note: "standup!", tagId: "tag-research" }, "cancel"));
        await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
        expect(mocked.timerStop).not.toHaveBeenCalled();
      });

      it("only stops the timer with Stop Timer when no time changed, and keeps the editor open", async () => {
        await openRunning();
        await userEvent.click(button("Stop Timer"));
        await waitFor(() => expect(mocked.timerStop).toHaveBeenCalled());
        expect(mocked.recordingUpdate).not.toHaveBeenCalled();
        expect(screen.getByRole("form", { name: "Edit recording" })).toBeInTheDocument();
      });

      it("saves the changed times alone before Stop Timer stops the timer, and keeps the editor open", async () => {
        const write: { finish?: (saved: Recording) => void } = {};
        mocked.recordingUpdate.mockImplementationOnce(() => new Promise((resolve) => (write.finish = resolve)));
        await openRunning();
        await userEvent.clear(field("Time 1 to"));
        await userEvent.type(field("Time 1 to"), "09:40");
        await waitFor(() => expect(field("Time 1 to")).toHaveValue("09:40"));
        await userEvent.type(field("Note"), "!");
        await userEvent.click(button("Stop Timer"));
        await waitFor(() => expect(write.finish).toBeDefined());
        expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", { spans: [{ start: morning.start, end: at("09:40") }] }, "cancel");
        expect(mocked.timerStop).not.toHaveBeenCalled();
        write.finish?.(savedRunning([{ start: morning.start, end: at("09:40") }], openSince));
        await waitFor(() => expect(mocked.timerStop).toHaveBeenCalled());
        expect(screen.getByRole("form", { name: "Edit recording" })).toBeInTheDocument();
      });

      it("saves a changed open start before Stop Timer stops the timer", async () => {
        await openRunning();
        await userEvent.clear(field("Time 2 from"));
        await userEvent.type(field("Time 2 from"), "11:10");
        await waitFor(() => expect(field("Time 2 from")).toHaveValue("11:10"));
        await userEvent.click(button("Stop Timer"));
        await waitFor(() => expect(mocked.timerStop).toHaveBeenCalled());
        expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", { openStart: at("11:10") }, "cancel");
      });

      it("shows the recording as stopped once the day shows its span closed", async () => {
        await openRunning();
        mocked.recordingsList.mockResolvedValue([recording({ ...standupRow, spans: [morning, { id: "r1-s2", start: openSince, end: at("11:50") }] })]);
        act(() => seedQuery(dayKey(DAY), dayView({ recordings: [{ ...standupBlock, end: at("11:50") }] })));
        await waitFor(() => expect(field("Time 2 to")).toHaveValue("11:50"));
        expect(screen.queryByRole("button", { name: "Stop Timer" })).not.toBeInTheDocument();
        expect(screen.queryByText("– running")).not.toBeInTheDocument();
      });

      describe("the open start", () => {
        const fields = { projectId: "p-site", note: "standup", tagId: "tag-research" };

        it("changes only the draft when a valid time is typed, and Save writes it alone with the fields", async () => {
          await openRunning();
          await userEvent.clear(field("Time 2 from"));
          await userEvent.type(field("Time 2 from"), "11:10");
          await waitFor(() => expect(field("Time 2 from")).toHaveValue("11:10"));
          await act(async () => {});
          expect(mocked.recordingUpdate).not.toHaveBeenCalled();
          await userEvent.click(button("Save"));
          await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", { ...fields, openStart: at("11:10") }, "cancel"));
          expect(await screen.findByText("Recording changed")).toBeInTheDocument();
          await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
        });

        it("steps by the snap interval with the arrow keys", async () => {
          await openRunning({ settings: settings({ timeline: { ...settings().timeline, snapMinutes: 15 } }) });
          await userEvent.type(field("Time 2 from"), "{ArrowUp}");
          expect(field("Time 2 from")).toHaveValue("11:15");
          await userEvent.type(field("Time 2 from"), "{ArrowDown}{ArrowDown}");
          expect(field("Time 2 from")).toHaveValue("10:45");
          expect(mocked.recordingUpdate).not.toHaveBeenCalled();
          await userEvent.click(button("Save"));
          await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalledWith("r1", { ...fields, openStart: at("10:45") }, "cancel"));
        });

        it("is dropped by Cancel", async () => {
          await openRunning();
          await userEvent.clear(field("Time 2 from"));
          await userEvent.type(field("Time 2 from"), "11:10");
          await waitFor(() => expect(field("Time 2 from")).toHaveValue("11:10"));
          await userEvent.click(button("Cancel"));
          expect(screen.queryByRole("form")).not.toBeInTheDocument();
          expect(mocked.recordingUpdate).not.toHaveBeenCalled();
        });

        it("stays in the editor with a toast when Save fails", async () => {
          await openRunning();
          mocked.recordingUpdate.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
          await userEvent.clear(field("Time 2 from"));
          await userEvent.type(field("Time 2 from"), "11:10");
          await waitFor(() => expect(field("Time 2 from")).toHaveValue("11:10"));
          await userEvent.click(button("Save"));
          expect(await screen.findByText("Could not change the recording: Disk full")).toBeInTheDocument();
          expect(screen.getByRole("form", { name: "Edit recording" })).toBeInTheDocument();
          expect(field("Time 2 from")).toHaveValue("11:10");
        });
      });

      describe("the closed spans", () => {
        it("change only the draft when a time is typed, and Save writes them without the open start", async () => {
          await openRunning();
          await userEvent.clear(field("Time 1 to"));
          await userEvent.type(field("Time 1 to"), "09:40");
          await waitFor(() => expect(field("Time 1 to")).toHaveValue("09:40"));
          expect(field("Time 2 from")).toHaveValue("11:00");
          expect(mocked.recordingUpdate).not.toHaveBeenCalled();
          await userEvent.click(button("Save"));
          await waitFor(() =>
            expect(mocked.recordingUpdate).toHaveBeenCalledWith(
              "r1",
              { projectId: "p-site", note: "standup", tagId: "tag-research", spans: [{ start: morning.start, end: at("09:40") }] },
              "cancel",
            ),
          );
        });

        it("get another row from Add time while the open span stays last, and Save writes it", async () => {
          await openRunning();
          await userEvent.click(button("Add time"));
          expect(await screen.findByLabelText("Time 3 from")).toHaveValue("11:00");
          expect(field("Time 2 from")).toHaveValue("09:30");
          expect(mocked.recordingUpdate).not.toHaveBeenCalled();
          await userEvent.click(button("Save"));
          await waitFor(() =>
            expect(mocked.recordingUpdate).toHaveBeenCalledWith(
              "r1",
              {
                projectId: "p-site",
                note: "standup",
                tagId: "tag-research",
                spans: [
                  { start: morning.start, end: morning.end },
                  { start: morning.end, end: morning.end + 30 * MINUTE_MS },
                ],
              },
              "cancel",
            ),
          );
        });

        it("show a ✕ on a single closed span, as the open span makes two", async () => {
          await openRunning();
          expect(button("Remove time 09:00–09:30")).toBeInTheDocument();
        });
      });

      describe("logged time", () => {
        afterEach(() => vi.useRealTimers());

        it("adds the time since the running span started to the closed spans", async () => {
          vi.useFakeTimers({ toFake: ["Date"] });
          vi.setSystemTime(at("12:20"));
          renderEditor({ mode: "edit", id: "r1" });
          await editorForm("Edit recording");
          expect(field("Hours")).toHaveValue("1");
          expect(field("Minutes")).toHaveValue("50");
        });
      });
    });

    it("keeps Save for a recording without a running span, offers no Stop Timer and does not stop the timer", async () => {
      mocked.recordingsList.mockResolvedValue([standupRow]);
      renderEditor({ mode: "edit", id: "r1" });
      await editorForm("Edit recording");
      expect(screen.queryByRole("button", { name: "Stop Timer" })).not.toBeInTheDocument();
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.recordingUpdate).toHaveBeenCalled());
      await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
      expect(mocked.timerStop).not.toHaveBeenCalled();
    });

    it("shows nothing and an error when the recording cannot be loaded", async () => {
      mocked.recordingsList.mockRejectedValueOnce({ kind: "db", message: "Database locked", code: null });
      renderEditor({ mode: "edit", id: "r1" });
      expect(await screen.findByText("Could not open the recording: Database locked")).toBeInTheDocument();
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
    });
  });

  describe("a running suggestion", () => {
    const openRunning = () =>
      renderEditor({ mode: "create", start: START, end: END, openStart: START, projectId: "p-site", note: "standup", suggestion: { id: "stored:st-1", by: "cli", running: { storedId: "st-1" } } });

    it("opens with its start as the running span and no closed spans", async () => {
      openRunning();
      await editorForm("Edit suggestion");
      expect(field("Time 1 from")).toHaveValue("09:00");
      expect(screen.getByText("– running")).toBeInTheDocument();
      expect(screen.queryByLabelText("Time 1 to")).not.toBeInTheDocument();
    });

    it("explains that Save keeps it a suggestion, with Save instead of Approve, and offers no Stop Timer or Delete", async () => {
      openRunning();
      const form = within(await editorForm("Edit suggestion"));
      expect(form.getByRole("note")).toHaveTextContent("Suggested by cli, still running. Save keeps it a suggestion; the timer moves only its end.");
      expect(form.getByRole("button", { name: "Save" })).toBeInTheDocument();
      expect(form.queryByRole("button", { name: "Approve" })).not.toBeInTheDocument();
      expect(form.queryByRole("button", { name: "Stop Timer" })).not.toBeInTheDocument();
      expect(form.queryByRole("button", { name: "Delete" })).not.toBeInTheDocument();
    });

    it("saves the fields to the stored suggestion, closes and offers Undo", async () => {
      openRunning();
      await editorForm("Edit suggestion");
      await userEvent.type(field("Note"), " notes");
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.suggestionUpdate).toHaveBeenCalledWith("st-1", { projectId: "p-site", note: "standup notes", tagId: null }));
      expect(await screen.findByText("Suggestion changed")).toBeInTheDocument();
      expect(button("Undo")).toBeInTheDocument();
      expect(screen.queryByRole("form")).not.toBeInTheDocument();
      expect(mocked.suggestionApproveEdited).not.toHaveBeenCalled();
      expect(mocked.recordingCreate).not.toHaveBeenCalled();
    });

    it("sends the new start as openStart when the start field was changed", async () => {
      openRunning();
      await editorForm("Edit suggestion");
      await userEvent.clear(field("Time 1 from"));
      await userEvent.type(field("Time 1 from"), "08:30");
      await userEvent.click(button("Save"));
      await waitFor(() => expect(mocked.suggestionUpdate).toHaveBeenCalledWith("st-1", { projectId: "p-site", note: "standup", tagId: null, openStart: at("08:30") }));
    });

    it("toasts a failed save and keeps the editor open", async () => {
      mocked.suggestionUpdate.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      openRunning();
      await editorForm("Edit suggestion");
      await userEvent.click(button("Save"));
      expect(await screen.findByText("Could not change the suggestion: Disk full")).toBeInTheDocument();
      expect(screen.getByRole("form", { name: "Edit suggestion" })).toBeInTheDocument();
    });

    it("shows the error of the start field under the times and keeps the editor open", async () => {
      mocked.suggestionUpdate.mockRejectedValueOnce({ kind: "invalid_input", message: "`start`: in the future", code: null, field: { path: "start", message: "The start is in the future" } });
      openRunning();
      await editorForm("Edit suggestion");
      await userEvent.click(button("Save"));
      expect(within(screen.getByRole("group", { name: "Times" })).getByText("The start is in the future")).toBeInTheDocument();
      expect(screen.getByRole("form", { name: "Edit suggestion" })).toBeInTheDocument();
    });
  });

  describe("a finished suggestion", () => {
    it("is approved with Approve and not saved to a stored suggestion", async () => {
      renderEditor({ mode: "create", start: START, end: END, projectId: "p-site", suggestion: { id: "s1", by: "Planner" } });
      const form = within(await editorForm("Edit suggestion"));
      expect(form.getByRole("note")).toHaveTextContent("Suggested by Planner. Approve records it.");
      await userEvent.click(form.getByRole("button", { name: "Approve" }));
      await waitFor(() => expect(mocked.suggestionApproveEdited).toHaveBeenCalledWith(DAY, "s1", expect.objectContaining({ projectId: "p-site" }), "cancel"));
      expect(mocked.suggestionUpdate).not.toHaveBeenCalled();
    });
  });
});
