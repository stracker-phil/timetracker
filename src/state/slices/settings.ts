import { setOverrides } from "../../commands/registry";
import { api, toAppError, type AppSettingPatch, type Keybindings, type Settings } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface SettingsSlice {
  settings: Settings | null;
  keybindings: Keybindings;
  loadSettings: () => Promise<void>;
  /** `quiet` skips the toast, for settings that apply as they change. */
  saveSettings: (settings: Settings, opts?: { quiet?: boolean }) => Promise<void>;
  saveKeybindings: (bindings: Keybindings) => Promise<void>;
  /** Resolves the reason when Rust refuses it (the system took the keys, or they are no chord). */
  saveTimerShortcut: (binding: string) => Promise<string | null>;
  /** One app's timeline override; resolves whether it was saved (a failure is toasted). */
  setAppSetting: (bundleId: string, patch: AppSettingPatch) => Promise<boolean>;
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
