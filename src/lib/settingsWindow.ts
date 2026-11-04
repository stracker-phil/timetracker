/** Whether this page is the Settings window (`windows/settings/`). */
let settingsWindow = false;

/** The Settings window marks itself while it is mounted; returns the reset. */
export function markSettingsWindow(): () => void {
  settingsWindow = true;
  return () => void (settingsWindow = false);
}

export const isSettingsWindow = () => settingsWindow;
