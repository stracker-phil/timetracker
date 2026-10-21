import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ActivityCount, ActivityInput, ActivityRow, DateJump, DeleteResult, AppError, AppSeen, AppSettingPatch, DayTotals, DayView, KnownApp, ReportFilter, ReportResult, SavedReport, WeekView, MonthView, TimerStatus, RecentTimer, AwayPrompt, ListView, Rule, RuleInput, RulePreview, Overlap, Recording, RecordingInput, RecordingPatch, Span, UndoStatus, AwayRow, BackupWritten, Client, ClientInput, Project, ProjectInput, ProjectListItem, Tag, TagListItem, DataChanged, DataInfo, ErrorKind, Keybindings, LaunchEvent, MenuState, PauseUntil, Period, PeriodUnit, PermissionStatus, PickerMonth, Range, RemoteEvent, Settings, TrackerStatus, UiState } from "./types";

export * from "./types";

/** The only place that talks to Rust. Tests mock this module. */
export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  /** Returns the settings as Rust saved them (clamped by `Settings::sanitized`). */
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  getKeybindings: () => invoke<Keybindings>("get_keybindings"),
  setKeybindings: (bindings: Keybindings) => invoke<Keybindings>("set_keybindings", { bindings }),
  getUiState: () => invoke<UiState>("get_ui_state"),
  setUiState: (ui: UiState) => invoke<UiState>("set_ui_state", { ui }),
  /** Marks the welcome sheet finished in `ui_state.json`. */
  onboardingDone: () => invoke<void>("onboarding_done"),
  /** Installs or removes the launch-at-login LaunchAgent and stores the choice; resolves whether it is on now. */
  autostartSet: (enabled: boolean) => invoke<boolean>("autostart_set", { enabled }),
  /** Whether the LaunchAgent is installed (the truth; `settings.launchAtLogin` mirrors it). */
  autostartStatus: () => invoke<boolean>("autostart_status"),
  openLogFolder: () => invoke<void>("open_log_folder"),
  /** Opens the bundled README (in debug builds the repository's) in the default Markdown app. */
  openReadme: () => invoke<void>("open_readme"),
  /** Reports which native menu items can run now and which are checked. */
  menuSync: (state: MenuState) => invoke<void>("menu_sync", { state }),
  /** http(s) links only; Rust refuses anything else. */
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  logError: (message: string) => invoke<void>("log_frontend_error", { message }),
  takeLaunchEvents: () => invoke<LaunchEvent[]>("take_launch_events"),
  /** Answers a `remote` event; a non-null `error` fails the request. */
  remoteReply: (id: string, result: unknown, error: string | null) => invoke<void>("remote_reply", { id, result, error }),
  /** Hides the window; the app keeps running in the menu bar. */
  hideWindow: () => invoke<void>("hide_window"),
  quitApp: () => invoke<void>("quit_app"),
  /** The native save panel; `null` when the user cancels. */
  chooseSavePath: (fileName: string, extension: string) => invoke<string | null>("choose_save_path", { fileName, extension }),
  /** Opens the enclosing folder in Finder with `path` selected. */
  chooseFolder: () => invoke<string | null>("choose_folder"),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  /** Where the data lives and what it holds; never rejects, a broken store reports `available: false`. */
  dataInfo: () => invoke<DataInfo>("data_info"),
  dataMove: (path: string) => invoke<DataInfo>("data_move", { path }),
  backupExport: (path: string) => invoke<BackupWritten>("backup_export", { path }),
  /** Rejects with `db` when the data store is not available, so the tracker is off. */
  trackerStatus: () => invoke<TrackerStatus>("tracker_status"),
  trackerPause: (until: PauseUntil) => invoke<TrackerStatus>("tracker_pause", { until }),
  trackerResume: () => invoke<TrackerStatus>("tracker_resume"),
  /** Shows the system prompt once per run, then reports the permission as it is now. */
  requestPermission: () => invoke<PermissionStatus>("request_permission"),
  /** System Settings → Privacy & Security → Accessibility. */
  openPrivacySettings: () => invoke<void>("open_privacy_settings"),
  /** The day's rows cut to the day; open rows end now. */
  activityList: (day: string) => invoke<ActivityRow[]>("activity_list", { day }),
  awayList: (day: string) => invoke<AwayRow[]>("away_list", { day }),
  /** The Day view of a day (`YYYY-MM-DD`): lanes of packed blocks, away spans, gaps, totals. */
  dayView: (day: string) => invoke<DayView>("day_view", { day }),
  /** Recorded, tracked and away time of a day, without its view. */
  dayTotals: (day: string) => invoke<DayTotals>("day_totals", { day }),
  /** Saves a new recording; Rust snaps its times. `overlap: "cancel"` rejects with kind `overlap` (`details`: the recordings in the way). */
  recordingCreate: (input: RecordingInput, overlap: Overlap) => invoke<Recording>("recording_create", { input, overlap }),
  recordingUpdate: (id: string, patch: RecordingPatch, overlap: Overlap) => invoke<Recording>("recording_update", { id, patch, overlap }),
  recordingDelete: (id: string) => invoke<void>("recording_delete", { id }),
  /** Recordings sharing time with `[from, to)`. */
  recordingsList: (from: number, to: number) => invoke<Recording[]>("recordings_list", { from, to }),
  /** The span as a write would store it, for a drag ghost. */
  recordingSnap: (start: number, end: number) => invoke<Span>("recording_snap", { start, end }),
  /** The span New Recording opens on without a selection: the half hour up to now, snapped. */
  recordingDefaultSpan: () => invoke<Span>("recording_default_span"),
  /** Undoes the last recording change; its label, or null when there was none. */
  undo: () => invoke<string | null>("undo"),
  redo: () => invoke<string | null>("redo"),
  undoStatus: () => invoke<UndoStatus>("undo_status"),
  /** Changes one app's timeline override (own lane, hidden); returns the saved settings. */
  setAppSetting: (bundleId: string, patch: AppSettingPatch) => invoke<Settings>("app_settings_set", { bundleId, patch }),
  /** Apps the tracker recorded in the last `days` days, most time first. */
  appsSeen: (days: number) => invoke<AppSeen[]>("apps_seen", { days }),
  appsList: () => invoke<KnownApp[]>("apps_list"),
  /** A running timer stops where the new one starts. */
  timerStart: (projectId: string, note: string, tagId: string | null) => invoke<Recording>("timer_start", { projectId, note, tagId }),
  /** `null` when no timer ran. */
  timerStop: () => invoke<Recording | null>("timer_stop"),
  timerStatus: () => invoke<TimerStatus>("timer_status"),
  timersRecent: () => invoke<RecentTimer[]>("timers_recent"),
  timerPickerOpen: () => invoke<null>("timer_picker_open"),
  timerPickerClose: () => invoke<null>("timer_picker_close"),
  weekView: (day: string) => invoke<WeekView>("week_view", { day }),
  monthView: (day: string) => invoke<MonthView>("month_view", { day }),
  /** The day, week or month that holds `day`: its first and last day, and whether it holds today. */
  calendarPeriod: (unit: PeriodUnit, day: string) => invoke<Period>("calendar_period", { unit, day }),
  /** The whole weeks covering `month`'s month, the days `selected` spans marked. */
  calendarPicker: (month: string, selected: [string, string] | null) => invoke<PickerMonth>("calendar_picker", { month, selected }),
  /** The palette's dates for `text`: a typed date, or the date words it starts; "" gives the Go to defaults. */
  calendarJumps: (text: string) => invoke<DateJump[]>("calendar_jumps", { text }),
  /** To `day` at the same time of day; Rust keeps the length. */
  recordingMove: (id: string, day: string, overlap: Overlap) => invoke<Recording>("recording_move", { id, day, overlap }),
  recordingCopy: (id: string, day: string, overlap: Overlap) => invoke<Recording>("recording_copy", { id, day, overlap }),
  reportRun: (filter: ReportFilter) => invoke<ReportResult>("report_run", { filter }),
  /** The period of the range's unit (month, week or its days) `by` steps away, from `anchor`'s period when given; presets where they match. */
  reportRangeShift: (range: Range, anchor: string | null, by: number) => invoke<Range>("report_range_shift", { range, anchor, by }),
  /** The report as the plain text the CLI prints. */
  reportText: (filter: ReportFilter) => invoke<string>("report_text", { filter }),
  savedReportsList: () => invoke<SavedReport[]>("saved_reports_list"),
  savedReportSave: (name: string, filter: ReportFilter) => invoke<SavedReport>("saved_report_save", { name, filter }),
  savedReportRename: (id: string, name: string) => invoke<SavedReport>("saved_report_rename", { id, name }),
  savedReportDelete: (id: string) => invoke<null>("saved_report_delete", { id }),
  /** Closed spans of other apps, all or none; returns their ids. */
  activityAdd: (items: ActivityInput[]) => invoke<number[]>("activity_add", { items }),
  /** Opens a span of `input.source`, closing its open one. */
  activityStart: (input: ActivityInput) => invoke<ActivityRow>("activity_start", { input }),
  /** Closes the source's open span at `end` (now when `null`); `null` when none is open. */
  activityStop: (source: string, end: number | null) => invoke<ActivityRow | null>("activity_stop", { source, end }),
  /** The names of the sources that reported activity, the latest first. */
  activitySources: () => invoke<string[]>("activity_sources"),
  /** How many activity and away spans share time with `[from, to)`: what `activityDelete` would remove. */
  activityCount: (from: number, to: number) => invoke<ActivityCount>("activity_count", { from, to }),
  /** Deletes those spans whole (recordings stay); `activityDeleteUndo` restores them until `undoUntil`. */
  activityDelete: (from: number, to: number) => invoke<DeleteResult>("activity_delete", { from, to }),
  /** Restores the last delete within 30 s of it; `not_found` after. */
  activityDeleteUndo: () => invoke<DeleteResult>("activity_delete_undo"),
  clientsList: (includeArchived: boolean) => invoke<Client[]>("clients_list", { includeArchived }),
  /** `id: null` creates; a taken or blank name rejects with `field.path` `name`. */
  clientsSave: (id: string | null, input: ClientInput) => invoke<Client>("clients_save", { id, input }),
  /** Archiving a client archives its projects too; restoring restores the client only. */
  clientsArchive: (id: string, archived: boolean) => invoke<Client>("clients_archive", { id, archived }),
  projectsList: (includeArchived: boolean) => invoke<ProjectListItem[]>("projects_list", { includeArchived }),
  /** `id: null` creates; validation errors name the field (`name`, `color`, `clientId`). */
  projectsSave: (id: string | null, input: ProjectInput) => invoke<Project>("projects_save", { id, input }),
  projectsArchive: (id: string, archived: boolean) => invoke<Project>("projects_archive", { id, archived }),
  /** Active projects by their latest recording. */
  projectsRecent: (limit: number) => invoke<Project[]>("projects_recent", { limit }),
  /** Every tag by name, with how many recordings carry it. */
  tagsList: () => invoke<TagListItem[]>("tags_list"),
  /** `id: null` creates, or returns the tag that has the name already. */
  tagsSave: (id: string | null, name: string) => invoke<Tag>("tags_save", { id, name }),
  /** Also removes the tag from every recording. */
  tagsDelete: (id: string) => invoke<void>("tags_delete", { id }),

  /** The open away prompt, `null` when there is none. */
  awayPromptCurrent: () => invoke<AwayPrompt | null>("away_prompt_current"),
  /** Records the answer and closes the window; an `overlap` keeps the prompt open. */
  awayPromptAnswer: (input: RecordingInput) => invoke<Recording>("away_prompt_answer", { input }),
  awayPromptSkip: () => invoke<void>("away_prompt_skip"),
  /** The day's activity grouped as the timeline labels it, longest first. */
  listView: (day: string) => invoke<ListView>("list_view", { day }),

  /** In position order: the first matching rule wins. */
  rulesList: () => invoke<Rule[]>("rules_list"),
  /** `id: null` creates; validation errors name the field (`name`, `projectId`, `note`, `tagId`, `conditions`, `conditions.<i>.value`). */
  ruleSave: (id: string | null, input: RuleInput) => invoke<Rule>("rule_save", { id, input }),
  ruleDelete: (id: string) => invoke<void>("rule_delete", { id }),
  /** `ids`: every rule, in the new order. */
  rulesReorder: (ids: string[]) => invoke<Rule[]>("rules_reorder", { ids }),
  ruleSetEnabled: (id: string, enabled: boolean) => invoke<Rule>("rule_set_enabled", { id, enabled }),
  /** What the input's conditions match over the last `days` days, today included. */
  rulePreview: (input: RuleInput, days: number) => invoke<RulePreview>("rule_preview", { input, days }),
  /** Records a suggestion of the day's view; `not_found` once it is gone, `overlap` when a recording took its time. */
  suggestionAccept: (day: string, id: string) => invoke<Recording>("suggestion_accept", { day, id }),
  /** Records every suggestion of the rule on the day as one undoable change. */
  suggestionsAcceptRule: (day: string, ruleId: string) => invoke<Recording[]>("suggestions_accept_rule", { day, ruleId }),

  onLaunch: (handler: () => void): Promise<UnlistenFn> => listen("launch", handler),
  /** Native menu and tray items emit the id of the command they run. */
  onCommand: (handler: (id: string) => void): Promise<UnlistenFn> => listen<string>("command", (e) => handler(e.payload)),
  /** Requests from other processes; answer each with `remoteReply`. */
  onRemote: (handler: (event: RemoteEvent) => void): Promise<UnlistenFn> => listen<RemoteEvent>("remote", (e) => handler(e.payload)),
  /** The tracker's state changed, or its heartbeat (at least every 10 s while it runs). */
  onTrackerStatus: (handler: (status: TrackerStatus) => void): Promise<UnlistenFn> => listen<TrackerStatus>("tracker_status", (e) => handler(e.payload)),
  /** Data of one day (or every day, `day: null`) changed in Rust; views showing it reload. */
  onDataChanged: (handler: (event: DataChanged) => void): Promise<UnlistenFn> => listen<DataChanged>("data_changed", (e) => handler(e.payload)),
  /** The away window: a new away span replaced the open prompt's. */
  onAwayPromptChanged: (handler: (prompt: AwayPrompt) => void): Promise<UnlistenFn> => listen<AwayPrompt>("away_prompt_changed", (e) => handler(e.payload)),
  onTimerStatus: (handler: (status: TimerStatus) => void): Promise<UnlistenFn> => listen<TimerStatus>("timer_status", (e) => handler(e.payload)),
};

/** Every `ErrorKind` Rust sends; the `Record` makes the compiler check the list is complete. */
const ERROR_KINDS: Record<ErrorKind, true> = {
  db: true,
  io: true,
  invalid_input: true,
  not_found: true,
  overlap: true,
  permission_required: true,
  tracker_paused: true,
  session_not_found: true,
  cancelled: true,
};

export function toAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "kind" in e && "message" in e && Object.hasOwn(ERROR_KINDS, String(e.kind))) {
    return e as AppError;
  }
  const message = e && typeof e === "object" && "message" in e ? String(e.message) : String(e);
  return { kind: "io", message, code: null };
}
