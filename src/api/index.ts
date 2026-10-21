import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppError, BackupWritten, DataChanged, DataInfo, ErrorKind, Keybindings, LaunchEvent, RemoteEvent, Settings, UiState } from "./types";

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
  openLogFolder: () => invoke<void>("open_log_folder"),
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
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  /** Where the data lives and what it holds; never rejects, a broken store reports `available: false`. */
  dataInfo: () => invoke<DataInfo>("data_info"),
  backupExport: (path: string) => invoke<BackupWritten>("backup_export", { path }),

  onLaunch: (handler: () => void): Promise<UnlistenFn> => listen("launch", handler),
  /** Native menu and tray items emit the id of the command they run. */
  onCommand: (handler: (id: string) => void): Promise<UnlistenFn> => listen<string>("command", (e) => handler(e.payload)),
  /** Requests from other processes; answer each with `remoteReply`. */
  onRemote: (handler: (event: RemoteEvent) => void): Promise<UnlistenFn> => listen<RemoteEvent>("remote", (e) => handler(e.payload)),
  /** Data of one day (or every day, `day: null`) changed in Rust; views showing it reload. */
  onDataChanged: (handler: (event: DataChanged) => void): Promise<UnlistenFn> => listen<DataChanged>("data_changed", (e) => handler(e.payload)),
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
