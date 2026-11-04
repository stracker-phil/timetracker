import { useEffect } from "react";
import { api, type Keybindings, type Settings } from "../api";
import { setOverrides } from "../commands/registry";
import { disposeAll } from "./disposeAll";
import { useStore } from "../state/store";
import { applyAppearance, useSystemDark } from "./theme";

/** A change saved in any window (`settings_changed`). */
export const settingsChanged = (settings: Settings) => useStore.setState({ settings });

/** Rebound shortcuts (`keybindings_changed`): the labels and the keys follow them. */
export function keybindingsChanged(keybindings: Keybindings) {
  useStore.setState({ keybindings });
  setOverrides(keybindings);
}

/** Loads settings and shortcuts once (then `onLoaded`), follows the changes Rust announces and applies the look. */
export function useFollowSettings(onLoaded?: () => void) {
  const settings = useStore((s) => s.settings);
  const systemDark = useSystemDark();
  useEffect(() => {
    void useStore.getState().loadSettings().then(onLoaded);
    const subscriptions = [api.onSettingsChanged(settingsChanged), api.onKeybindingsChanged(keybindingsChanged)];
    return () => disposeAll(subscriptions);
    // Loads once per window; `onLoaded` is the caller's setup, not a dependency.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    if (settings) applyAppearance(settings, systemDark);
  }, [settings, systemDark]);
}
