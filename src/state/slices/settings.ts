import { setOverrides } from "../../commands/registry";
import { api, toAppError, type AppSettingPatch, type Keybindings, type Settings, type TimelineSettings } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface SettingsSlice {
  settings: Settings | null;
  keybindings: Keybindings;
  loadSettings: () => Promise<void>;
  /** `quiet` skips the toast, for settings that apply as they change. */
  saveSettings: (settings: Settings, opts?: { quiet?: boolean }) => Promise<void>;
  /** Changes timeline settings and saves them quietly; the Day view reloads. */
  saveTimeline: (patch: Partial<TimelineSettings>) => Promise<void>;
  saveKeybindings: (bindings: Keybindings) => Promise<void>;
  /** Resolves the reason when Rust refuses it (the system took the keys, or they are no chord). */
  saveTimerShortcut: (binding: string) => Promise<string | null>;
  /** One app's timeline override; resolves whether it was saved (a failure is toasted). */
  setAppSetting: (bundleId: string, patch: AppSettingPatch) => Promise<boolean>;
  /** Turns launch at login on or off; resolves whether it is on now, or `null` when it failed (toasted). */
  setLaunchAtLogin: (enabled: boolean) => Promise<boolean | null>;
}

export const settingsSlice: Slice<SettingsSlice> = {
  create: (set, get) => ({
    settings: null,
    keybindings: {},

    async loadSettings() {
      try {
        const [settings, keybindings] = await Promise.all([api.getSettings(), api.getKeybindings().catch(() => ({}))]);
        set({ settings, keybindings });
      } catch (e) {
        fail(e, "Could not load settings");
      }
    },

    async saveSettings(settings, opts) {
      try {
        set({ settings: await api.updateSettings(settings) });
        if (!opts?.quiet) get().toast("success", "Settings saved");
      } catch (e) {
        fail(e, "Could not save settings");
      }
    },

    async saveTimerShortcut(binding) {
      const settings = get().settings;
      if (!settings) return null;
      try {
        set({ settings: await api.updateSettings({ ...settings, timerShortcut: binding }) });
        return null;
      } catch (e) {
        return toAppError(e).message;
      }
    },

    async setAppSetting(bundleId, patch) {
      try {
        set({ settings: await api.setAppSetting(bundleId, patch) });
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
        const settings = get().settings;
        if (settings) set({ settings: { ...settings, launchAtLogin } });
        return launchAtLogin;
      } catch (e) {
        fail(e, "Could not change launch at login");
        return null;
      }
    },

    async saveTimeline(patch) {
      const { settings } = get();
      if (settings) await get().saveSettings({ ...settings, timeline: { ...settings.timeline, ...patch } }, { quiet: true });
    },

    async saveKeybindings(bindings) {
      try {
        const saved = await api.setKeybindings(bindings);
        set({ keybindings: saved });
        setOverrides(saved);
      } catch (e) {
        fail(e, "Could not save shortcuts");
      }
    },
  }),
};
