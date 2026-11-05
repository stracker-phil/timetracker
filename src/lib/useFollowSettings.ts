import { useEffect } from "react";
import { loadSettings, useSettings } from "../queries/settings";
import { useEvents } from "../queries/useEvents";
import { applyAppearance, useSystemDark } from "./theme";

/**
 * A secondary window's setup: loads settings and shortcuts once (then `onLoaded`), follows what Rust announces
 * (`useEvents`, so a change saved in any window shows) and applies the look.
 */
export function useFollowSettings(onLoaded?: () => void) {
  const settings = useSettings();
  const systemDark = useSystemDark();
  useEvents();
  useEffect(() => {
    void loadSettings().then(onLoaded);
    // Loads once per window; `onLoaded` is the caller's setup, not a dependency.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    if (settings) applyAppearance(settings, systemDark);
  }, [settings, systemDark]);
}
