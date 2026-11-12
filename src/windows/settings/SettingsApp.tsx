import { useEffect, useState } from "react";
import { api } from "../../api";
import { installCommands } from "../../commands";
import { handleSettingsKeydown } from "../../commands/settingsWindow";
import { MenuHost } from "../../components/menu";
import { SettingsWindow, type SettingsRequest } from "../../components/settings/SettingsWindow";
import { Toasts } from "../../components/Toasts";
import { TooltipHost } from "../../components/TooltipHost";
import { disposeAll } from "../../lib/disposeAll";
import { markSettingsWindow } from "../../lib/settingsWindow";
import { useErrorToasts } from "../../lib/useErrorToasts";
import { useFollowSettings } from "../../lib/useFollowSettings";
import { trackWindowFocus } from "../../lib/windowFocus";
import { QueryRoot } from "../../queries/QueryRoot";

/** The page or row Rust opened the window at (`?page=`); later requests arrive as `open_settings`. */
function initialRequest(): SettingsRequest {
  return { id: new URLSearchParams(window.location.search).get("page") ?? undefined };
}

/** This window's keys (`settings` scope) and the inactive-window dimming. */
function useWindowSetup() {
  useEffect(() => {
    const unmark = markSettingsWindow();
    const untrack = trackWindowFocus();
    window.addEventListener("keydown", handleSettingsKeydown, true);
    return () => {
      unmark();
      untrack();
      window.removeEventListener("keydown", handleSettingsKeydown, true);
    };
  }, []);
}

function SettingsAppContent() {
  const [request, setRequest] = useState(initialRequest);
  const open = (id: string | undefined) => setRequest({ id });
  // Re-installing applies the user's shortcut overrides, so Shortcuts lists every command as bound.
  useFollowSettings(installCommands);
  useWindowSetup();
  useErrorToasts();
  useEffect(() => {
    const subscriptions = [api.onOpenSettings((id) => setRequest({ id }))];
    return () => disposeAll(subscriptions);
  }, []);

  return (
    <>
      <SettingsWindow request={request} onOpen={open} />
      <MenuHost />
      <Toasts />
      <TooltipHost />
    </>
  );
}

export function SettingsApp() {
  return (
    <QueryRoot>
      <SettingsAppContent />
    </QueryRoot>
  );
}
