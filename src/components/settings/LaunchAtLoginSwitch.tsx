import { useState } from "react";
import { useStore } from "../../state/store";
import { Switch } from "./controls";
import { useAutostartStatus } from "../../queries/system";

/** Shows whether the LaunchAgent is installed (not the stored setting) and switches it. */
export function LaunchAtLoginSwitch() {
  const { data: installed } = useAutostartStatus();
  const [changed, setChanged] = useState<boolean | null>(null);
  const change = async (enabled: boolean) => {
    const now = await useStore.getState().setLaunchAtLogin(enabled);
    if (now !== null) setChanged(now);
  };
  return (
    <Switch
      label="Launch at login"
      anchor="launch-at-login"
      description="Starts TimeTracker in the menu bar when you log in, without opening the window."
      checked={changed ?? installed ?? false}
      onChange={(enabled) => void change(enabled)}
    />
  );
}
