import { setOverrides } from "../../commands/registry";
import { api, toAppError, type AppSettingPatch, type Keybindings, type Settings, type TimelineSettings } from "../../api";
import { currentSettings, setKeybindingsData, setSettingsData } from "../../queries/settings";
import { fail } from "../requests";
import type { Slice } from "../store";

/** The writes of the settings and shortcuts `useSettings` and `useKeybindings` read (`src/queries/settings.ts`); each stores what Rust saved. */
export interface SettingsSlice {
  /** `quiet` skips the toast, for settings that apply as they change. With `returnError`, a value Rust refuses
   *  (`invalid_input`) is not toasted: its message resolves, for the field to show. Resolves null otherwise. */
  saveSettings: (settings: Settings, opts?: { quiet?: boolean; returnError?: boolean }) => Promise<string | null>;
  /** Changes timeline settings and saves them quietly; the Day view reloads. */
  saveTimeline: (patch: Partial<TimelineSettings>) => Promise<void>;
  saveKeybindings: (bindings: Keybindings) => Promise<void>;
  /** Resolves the reason when Rust refuses it (the system took the keys, or they are no chord). */
  saveTimerShortcut: (binding: string | null) => Promise<string | null>;
  /** One app's timeline override; resolves whether it was saved (a failure is toasted). */
  setAppSetting: (bundleId: string, patch: AppSettingPatch) => Promise<boolean>;
  /** Turns launch at login on or off; resolves whether it is on now, or `null` when it failed (toasted). */
  setLaunchAtLogin: (enabled: boolean) => Promise<boolean | null>;
}

export const settingsSlice: Slice<SettingsSlice> = {
  create: (_set, get) => ({
    async saveSettings(settings, opts) {
      try {
        setSettingsData(await api.updateSettings(settings));
        if (!opts?.quiet) get().toast("success", "Settings saved");
      } catch (e) {
        const error = toAppError(e);
        if (opts?.returnError && error.kind === "invalid_input") return error.message;
        fail(error, "Could not save settings");
      }
      return null;
    },

    async saveTimerShortcut(binding) {
      const settings = currentSettings();
      if (!settings) return null;
      try {
        setSettingsData(await api.updateSettings({ ...settings, timerShortcut: binding }));
        return null;
      } catch (e) {
        return toAppError(e).message;
      }
    },

    async setAppSetting(bundleId, patch) {
      try {
        setSettingsData(await api.setAppSetting(bundleId, patch));
        return true;
      } catch (e) {
        fail(e, "Could not change the app's timeline setting");
        return false;
      }
    },

    async setLaunchAtLogin(enabled) {
      try {
        const launchAtLogin = await api.autostartSet(enabled);
        // Rust stored the choice; the copy here must match, or the next settings save would undo it.
        const settings = currentSettings();
        if (settings) setSettingsData({ ...settings, launchAtLogin });
        return launchAtLogin;
      } catch (e) {
        fail(e, "Could not change launch at login");
        return null;
      }
    },

    async saveTimeline(patch) {
      const settings = currentSettings();
      if (settings) await get().saveSettings({ ...settings, timeline: { ...settings.timeline, ...patch } }, { quiet: true });
    },

    async saveKeybindings(bindings) {
      try {
        const saved = await api.setKeybindings(bindings);
        setKeybindingsData(saved);
        setOverrides(saved);
      } catch (e) {
        fail(e, "Could not save shortcuts");
      }
    },
  }),
};
