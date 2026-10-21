import { vi } from "vitest";
import type { api as realApi } from "../api";
// Must not import the store or "../api": they would load the real api module while it is being mocked.
import { EMPTY_UI_STATE, type ClientInput, type Keybindings, type ProjectInput, type RecordingInput, type RecordingPatch, type ReportFilter, type Settings, type UiState } from "../api/types";
import { mondayOf, monthView, weekView } from "./calendarFixtures";
import { dataInfo, trackerStatus } from "./data";
import { dayView } from "./dayFixtures";
import { client, DEFAULT_TAGS, project } from "./projectFixtures";
import { recording } from "./recordingFixtures";
import { reportResult, savedReport } from "./reportFixtures";
import { settings } from "./settings";

const noUnlisten = () => Promise.resolve(() => {});

/** An `api` where every call resolves to a valid default; override per test with mockResolvedValue. */
export function createApiMock(): typeof realApi {
  return {
    getSettings: vi.fn().mockResolvedValue(settings()),
    updateSettings: vi.fn(async (s: Settings) => s),
    getKeybindings: vi.fn().mockResolvedValue({}),
    setKeybindings: vi.fn(async (bindings: Keybindings) => bindings),
    getUiState: vi.fn().mockResolvedValue(EMPTY_UI_STATE),
    setUiState: vi.fn(async (ui: UiState) => ui),
    openLogFolder: vi.fn().mockResolvedValue(undefined),
    openUrl: vi.fn().mockResolvedValue(undefined),
    logError: vi.fn().mockResolvedValue(undefined),
    takeLaunchEvents: vi.fn().mockResolvedValue([]),
    remoteReply: vi.fn().mockResolvedValue(undefined),
    hideWindow: vi.fn().mockResolvedValue(undefined),
    quitApp: vi.fn().mockResolvedValue(undefined),
    chooseSavePath: vi.fn().mockResolvedValue(null),
    revealPath: vi.fn().mockResolvedValue(undefined),
    dataInfo: vi.fn().mockResolvedValue(dataInfo()),
    backupExport: vi.fn().mockResolvedValue({ bytes: 0, recordings: 0, activities: 0 }),
    trackerStatus: vi.fn().mockResolvedValue(trackerStatus()),
    trackerPause: vi.fn().mockResolvedValue(trackerStatus({ state: { kind: "paused", until: null }, text: "Paused" })),
    trackerResume: vi.fn().mockResolvedValue(trackerStatus()),
    requestPermission: vi.fn().mockResolvedValue("granted"),
    openPrivacySettings: vi.fn().mockResolvedValue(undefined),
    activityList: vi.fn().mockResolvedValue([]),
    awayList: vi.fn().mockResolvedValue([]),
    dayView: vi.fn(async (day: string) => dayView({ day })),
    weekView: vi.fn(async (day: string) => weekView({ weekStart: mondayOf(day) })),
    monthView: vi.fn(async (day: string) => monthView({ monthStart: `${day.slice(0, 7)}-01` })),
    recordingMove: vi.fn(async (id: string) => recording({ id })),
    recordingCopy: vi.fn(async () => recording({ id: "recording-copy" })),
    recordingCreate: vi.fn(async (input: RecordingInput) => recording({ ...input, id: "recording-new" })),
    recordingUpdate: vi.fn(async (id: string, patch: RecordingPatch) => recording({ ...patch, id })),
    recordingDelete: vi.fn().mockResolvedValue(undefined),
    recordingsList: vi.fn().mockResolvedValue([]),
    recordingSnap: vi.fn(async (start: number, end: number) => ({ start, end })),
    undo: vi.fn().mockResolvedValue(null),
    redo: vi.fn().mockResolvedValue(null),
    undoStatus: vi.fn().mockResolvedValue({ undoLabel: null, redoLabel: null }),
    setAppSetting: vi.fn(async () => settings()),
    appsList: vi.fn().mockResolvedValue([]),
    appsSeen: vi.fn().mockResolvedValue([]),
    activityAdd: vi.fn().mockResolvedValue([]),
    activityStart: vi.fn().mockRejectedValue({ kind: "invalid_input", message: "Not mocked", code: null }),
    activityStop: vi.fn().mockResolvedValue(null),
    activitySources: vi.fn().mockResolvedValue([]),
    clientsList: vi.fn().mockResolvedValue([]),
    clientsSave: vi.fn(async (id: string | null, input: ClientInput) => client({ ...input, id: id ?? "client-new" })),
    clientsArchive: vi.fn(async (id: string, archived: boolean) => client({ id, archivedAt: archived ? 1 : null })),
    projectsList: vi.fn().mockResolvedValue([]),
    projectsSave: vi.fn(async (id: string | null, input: ProjectInput) => project({ ...input, shade: input.shade ?? 4, id: id ?? "project-new" })),
    projectsArchive: vi.fn(async (id: string, archived: boolean) => project({ id, archivedAt: archived ? 1 : null })),
    projectsRecent: vi.fn().mockResolvedValue([]),
    tagsList: vi.fn().mockResolvedValue(DEFAULT_TAGS),
    tagsSave: vi.fn(async (id: string | null, name: string) => ({ id: id ?? `tag-${name}`, name, createdAt: 0 })),
    tagsDelete: vi.fn().mockResolvedValue(undefined),
    reportRun: vi.fn().mockResolvedValue(reportResult()),
    reportText: vi.fn().mockResolvedValue("Mon 2026-10-20\n\nNo recordings.\n"),
    savedReportsList: vi.fn().mockResolvedValue([]),
    savedReportSave: vi.fn(async (name: string, filter: ReportFilter) => savedReport({ name, filter })),
    savedReportRename: vi.fn(async (id: string, name: string) => savedReport({ id, name })),
    savedReportDelete: vi.fn().mockResolvedValue(null),
    timerStart: vi.fn(async (projectId: string, note: string, tagIds: string[]) => recording({ id: "recording-timer", projectId, note, tagIds, end: null })),
    timerStop: vi.fn().mockResolvedValue(null),
    timerStatus: vi.fn().mockResolvedValue({ running: null }),
    timersRecent: vi.fn().mockResolvedValue([]),
    timerPickerOpen: vi.fn().mockResolvedValue(null),
    timerPickerClose: vi.fn().mockResolvedValue(null),
    onLaunch: vi.fn(noUnlisten),
    onCommand: vi.fn(noUnlisten),
    onRemote: vi.fn(noUnlisten),
    onTrackerStatus: vi.fn(noUnlisten),
    onTimerStatus: vi.fn(noUnlisten),
    onDataChanged: vi.fn(noUnlisten),
  };
}
