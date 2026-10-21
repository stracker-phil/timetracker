import { useState } from "react";
import { api } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { useStore } from "../../state/store";
import { Switch } from "./controls";

/** Shows whether the LaunchAgent is installed (not the stored setting) and switches it. */
export function LaunchAtLoginSwitch() {
  const { data: installed } = useApiResult("autostart-status", api.autostartStatus);
  const [changed, setChanged] = useState<boolean | null>(null);
  const change = async (enabled: boolean) => {
    const now = await useStore.getState().setLaunchAtLogin(enabled);
    if (now !== null) setChanged(now);
  };
  return (
    <Switch
      label="Launch at login"
      description="Starts TimeTracker in the menu bar when you log in, without opening the window."
      checked={changed ?? installed ?? false}
      onChange={(enabled) => void change(enabled)}
    />
  );
}
