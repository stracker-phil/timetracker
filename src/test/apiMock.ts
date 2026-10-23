import { vi } from "vitest";
import type { api as realApi } from "../api";
// Must not import the store or "../api": they would load the real api module while it is being mocked.
import { EMPTY_UI_STATE, type ClientInput, type DateJump, type Keybindings, type PeriodUnit, type ProjectInput, type Range, type RecordingAutofill, type RecordingInput, type RecordingPatch, type RecordingSpan, type ReportFilter, type RuleInput, type Settings, type SnapEdge, type Span, type UiState } from "../api/types";
import { mondayOf, monthView, weekView } from "./calendarFixtures";
import { dataInfo, trackerStatus } from "./data";
import { dayView } from "./dayFixtures";
import { client, DEFAULT_TAGS, project } from "./projectFixtures";
import { recording } from "./recordingFixtures";
import { reportResult, savedReport } from "./reportFixtures";
import { rule, rulePreview } from "./ruleFixtures";
import { settings } from "./settings";

const noUnlisten = () => Promise.resolve(() => {});

/** Spans as Rust returns them: with ids. */
/** The editor's spans as Rust answers: as given, with their sum. */
const draftSpans = (spans: Span[]) => ({ spans, totalMs: spans.reduce((sum, s) => sum + s.end - s.start, 0) });

const sorted = (spans: Span[]) => [...spans].sort((a, b) => a.start - b.start);

const withIds = (recordingId: string, spans: Span[], from = 1): RecordingSpan[] => spans.map((s, i) => ({ id: `${recordingId}-s${from + i}`, ...s }));

/** What `calendar_step` answers: days and weeks by the calendar, a month step on the 1st. */
function stepDay(unit: PeriodUnit, day: string, by: number): string {
  const [y, m, d] = day.split("-").map(Number);
  const date = unit === "month" ? new Date(y, m - 1 + by, 1) : new Date(y, m - 1, d + (unit === "week" ? 7 * by : by));
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** What `calendar_parse_time` answers for a clock time (`9:30`, `0930`) on the local day of `on`, or a `YYYY-MM-DDTHH:MM` value on its own day. */
function parseClock(text: string, on: number): number | null {
  const match = /^(?:(\d{4})-(\d{2})-(\d{2})T)?(\d{1,2}):?(\d{2})$/.exec(text.trim());
  if (!match || Number(match[4]) > 23 || Number(match[5]) > 59) return null;
  const date = match[1] ? new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3])) : new Date(on);
  date.setHours(Number(match[4]), Number(match[5]), 0, 0);
  return date.getTime();
}

/** An `api` where every call resolves to a valid default; override per test with mockResolvedValue. */
export function createApiMock(): typeof realApi {
  return {
    getSettings: vi.fn().mockResolvedValue(settings()),
    updateSettings: vi.fn(async (s: Settings) => s),
    getKeybindings: vi.fn().mockResolvedValue({}),
    setKeybindings: vi.fn(async (bindings: Keybindings) => bindings),
    getUiState: vi.fn().mockResolvedValue(EMPTY_UI_STATE),
    setUiState: vi.fn(async (ui: UiState) => ui),
    onboardingDone: vi.fn().mockResolvedValue(undefined),
    autostartSet: vi.fn(async (enabled: boolean) => enabled),
    autostartStatus: vi.fn().mockResolvedValue(false),
    openLogFolder: vi.fn().mockResolvedValue(undefined),
    openReadme: vi.fn().mockResolvedValue(undefined),
    menuSync: vi.fn().mockResolvedValue(undefined),
    openUrl: vi.fn().mockResolvedValue(undefined),
    logError: vi.fn().mockResolvedValue(undefined),
    takeLaunchEvents: vi.fn().mockResolvedValue([]),
    remoteReply: vi.fn().mockResolvedValue(undefined),
    hideWindow: vi.fn().mockResolvedValue(undefined),
    hideToMenuBar: vi.fn().mockResolvedValue(undefined),
    quitApp: vi.fn().mockResolvedValue(undefined),
    chooseSavePath: vi.fn().mockResolvedValue(null),
    chooseFolder: vi.fn().mockResolvedValue(null),
    revealPath: vi.fn().mockResolvedValue(undefined),
    dataInfo: vi.fn().mockResolvedValue(dataInfo()),
    dataMove: vi.fn().mockResolvedValue(dataInfo()),
    backupExport: vi.fn().mockResolvedValue({ bytes: 0, recordings: 0, activities: 0 }),
    trackerStatus: vi.fn().mockResolvedValue(trackerStatus()),
    trackerPause: vi.fn().mockResolvedValue(trackerStatus({ state: { kind: "paused", until: null }, text: "Paused" })),
    trackerResume: vi.fn().mockResolvedValue(trackerStatus()),
    requestPermission: vi.fn().mockResolvedValue("granted"),
    openPrivacySettings: vi.fn().mockResolvedValue(undefined),
    activityList: vi.fn().mockResolvedValue([]),
    awayList: vi.fn().mockResolvedValue([]),
    dayView: vi.fn(async (day: string) => dayView({ day })),
    dayTotals: vi.fn().mockResolvedValue({ recordedMs: 0, trackedMs: 0, awayMs: 0 }),
    weekView: vi.fn(async (day: string) => weekView({ weekStart: mondayOf(day) })),
    monthView: vi.fn(async (day: string) => monthView({ monthStart: `${day.slice(0, 7)}-01` })),
    calendarPeriod: vi.fn(async (_unit: PeriodUnit, day: string) => ({ from: day, to: day, holdsToday: false })),
    calendarMiniMonth: vi.fn(async (shownDay: string) => ({ monthStart: `${shownDay.slice(0, 7)}-01`, monthName: "Oct", year: Number(shownDay.slice(0, 4)), monthSelected: false, weeks: [] })),
    calendarPicker: vi.fn(async (month: string) => ({ monthStart: `${month.slice(0, 7)}-01`, weeks: [] })),
    calendarJumps: vi.fn(async (): Promise<DateJump[]> => []),
    calendarStep: vi.fn(async (unit: PeriodUnit, day: string, by: number) => stepDay(unit, day, by)),
    calendarParseTime: vi.fn(async (text: string, on: number) => parseClock(text, on)),
    recordingMove: vi.fn(async (id: string) => recording({ id })),
    recordingCopy: vi.fn(async () => recording({ id: "recording-copy" })),
    recordingCreate: vi.fn(async (input: RecordingInput) => recording({ ...input, id: "recording-new", spans: withIds("recording-new", input.spans) })),
    recordingUpdate: vi.fn(async (id: string, patch: RecordingPatch) => {
      const { spans, ...rest } = patch;
      return recording({ ...rest, id, ...(spans ? { spans: withIds(id, spans) } : {}) });
    }),
    recordingAddSpan: vi.fn(async (id: string, span: Span) => recording({ id, spans: [...recording({ id }).spans, ...withIds(id, [span], 2)] })),
    recordingUpdateSpan: vi.fn(async (id: string, spanId: string, span: Span, _moving: SnapEdge | null) => recording({ id, spans: [{ id: spanId, ...span }] })),
    recordingRemoveSpan: vi.fn().mockResolvedValue(null),
    recordingSplitSpan: vi.fn(async (id: string) => recording({ id })),
    recordingMerge: vi.fn(async (into: string) => recording({ id: into })),
    recordingNudge: vi.fn(async (id: string) => recording({ id })),
    recordingDelete: vi.fn().mockResolvedValue(undefined),
    recordingsList: vi.fn().mockResolvedValue([]),
    recordingSnap: vi.fn(async (start: number, end: number) => ({ start, end })),
    recordingDraftSpans: vi.fn(async (spans: Span[]) => draftSpans(spans)),
    recordingDraftTotal: vi.fn(async (spans: Span[], totalMs: number) => {
      const last = spans[spans.length - 1];
      return draftSpans(last ? [...spans.slice(0, -1), { ...last, end: last.end + totalMs - draftSpans(spans).totalMs }] : spans);
    }),
    recordingDraftAdd: vi.fn(async (spans: Span[]) => {
      const start = spans.length ? spans[spans.length - 1].end : 0;
      return draftSpans([...spans, { start, end: start + 30 * 60_000 }]);
    }),
    recordingDraftInclude: vi.fn(async (spans: Span[], span: Span) => draftSpans(sorted([...spans, span]))),
    recordingDraftResize: vi.fn(async (spans: Span[], index: number, span: Span, _moving: SnapEdge | null) => draftSpans(sorted(spans.map((s, i) => (i === index ? span : s))))),
    recordingDraftToggle: vi.fn(async (spans: Span[], block: Span, _others: Span[]) => {
      const present = spans.some((s) => s.start === block.start && s.end === block.end);
      const next = present ? spans.filter((s) => s.start !== block.start || s.end !== block.end) : sorted([...spans, block]);
      return { ...draftSpans(next), included: !present };
    }),
    recordingProposal: vi.fn(async () => ({ projectId: null })),
    recordingAutofill: vi.fn(async (): Promise<RecordingAutofill | null> => null),
    recordingDefaultSpan: vi.fn().mockResolvedValue({ start: 0, end: 1_800_000 }),
    undo: vi.fn().mockResolvedValue(null),
    redo: vi.fn().mockResolvedValue(null),
    undoStatus: vi.fn().mockResolvedValue({ undoLabel: null, redoLabel: null }),
    setAppSetting: vi.fn(async () => settings()),
    appsList: vi.fn().mockResolvedValue([]),
    appsSeen: vi.fn().mockResolvedValue([]),
    appIcons: vi.fn().mockResolvedValue({}),
    activityAdd: vi.fn().mockResolvedValue([]),
    activityStart: vi.fn().mockRejectedValue({ kind: "invalid_input", message: "Not mocked", code: null }),
    activityStop: vi.fn().mockResolvedValue(null),
    activitySources: vi.fn().mockResolvedValue([]),
    activityCount: vi.fn().mockResolvedValue({ activities: 0, away: 0 }),
    activityDelete: vi.fn().mockResolvedValue({ activities: 0, away: 0, undoUntil: 30_000 }),
    activityDeleteUndo: vi.fn().mockResolvedValue({ activities: 0, away: 0, undoUntil: 30_000 }),
    clientsList: vi.fn().mockResolvedValue([]),
    clientsSave: vi.fn(async (id: string | null, input: ClientInput) => client({ ...input, id: id ?? "client-new" })),
    clientsArchive: vi.fn(async (id: string, archived: boolean) => client({ id, archivedAt: archived ? 1 : null })),
    projectsList: vi.fn().mockResolvedValue([]),
    projectsSave: vi.fn(async (id: string | null, input: ProjectInput) => project({ ...input, shade: input.shade ?? 4, id: id ?? "project-new" })),
    projectsArchive: vi.fn(async (id: string, archived: boolean) => project({ id, archivedAt: archived ? 1 : null })),
    projectsRecent: vi.fn().mockResolvedValue([]),
    tagsList: vi.fn().mockResolvedValue(DEFAULT_TAGS),
    tagsSave: vi.fn(async (id: string | null, name: string) => ({ id: id ?? `tag-${name}`, name, emoji: null, createdAt: 0 })),
    tagsSetEmoji: vi.fn(async (id: string, emoji: string | null) => ({ id, name: id, emoji, createdAt: 0 })),
    tagsDelete: vi.fn().mockResolvedValue(undefined),
    reportRun: vi.fn().mockResolvedValue(reportResult()),
    periodTotals: vi.fn().mockResolvedValue({ label: "Today", recordedMs: 0, billableMs: 0, billablePercent: null }),
    reportRangeShift: vi.fn(async (range: Range) => range),
    reportText: vi.fn().mockResolvedValue("Mon 2026-10-20\n\nNo recordings.\n"),
    savedReportsList: vi.fn().mockResolvedValue([]),
    savedReportSave: vi.fn(async (name: string, filter: ReportFilter) => savedReport({ name, filter })),
    savedReportRename: vi.fn(async (id: string, name: string) => savedReport({ id, name })),
    savedReportDelete: vi.fn().mockResolvedValue(null),
    timerStart: vi.fn(async (projectId: string, note: string, tagId: string | null) => recording({ id: "recording-timer", projectId, note, tagId, spans: [{ id: "recording-timer-s1", start: 0, end: null }] })),
    timerUpdate: vi.fn(async (projectId: string, note: string, tagId: string | null) => recording({ id: "recording-timer", projectId, note, tagId, spans: [{ id: "recording-timer-s1", start: 0, end: null }] })),
    timerStop: vi.fn().mockResolvedValue(null),
    timerStatus: vi.fn().mockResolvedValue({ running: null }),
    timersRecent: vi.fn().mockResolvedValue([]),
    timerPickerOpen: vi.fn().mockResolvedValue(null),
    timerPickerClose: vi.fn().mockResolvedValue(null),
    awayPromptCurrent: vi.fn().mockResolvedValue(null),
    awayPromptAnswer: vi.fn(async (input: RecordingInput) => recording({ ...input, id: "recording-away", spans: withIds("recording-away", input.spans) })),
    awayPromptSkip: vi.fn().mockResolvedValue(undefined),
    rulesList: vi.fn().mockResolvedValue([]),
    ruleSave: vi.fn(async (id: string | null, input: RuleInput) => rule({ ...input, id: id ?? "rule-new", conditions: input.conditions.map((c, i) => ({ ...c, id: `c${i}` })) })),
    ruleDelete: vi.fn().mockResolvedValue(undefined),
    rulesReorder: vi.fn().mockResolvedValue([]),
    ruleSetEnabled: vi.fn(async (id: string, enabled: boolean) => rule({ id, enabled })),
    rulePreview: vi.fn().mockResolvedValue(rulePreview()),
    suggestionApprove: vi.fn(async () => ({ recording: recording({ id: "recording-suggested" }), durationMs: 2_700_000 })),
    suggestionApproveRule: vi.fn().mockResolvedValue([]),
    suggestionsApprove: vi.fn().mockResolvedValue([]),
    suggestionApproveEdited: vi.fn(async () => recording()),
    suggestionsDismiss: vi.fn().mockResolvedValue(1),
    rulesToday: vi.fn().mockResolvedValue([]),
    onLaunch: vi.fn(noUnlisten),
    onWindowOpened: vi.fn(noUnlisten),
    onCommand: vi.fn(noUnlisten),
    onRemote: vi.fn(noUnlisten),
    onTrackerStatus: vi.fn(noUnlisten),
    onTimerStatus: vi.fn(noUnlisten),
    onDataChanged: vi.fn(noUnlisten),
    onAwayPromptChanged: vi.fn(noUnlisten),
  };
}
