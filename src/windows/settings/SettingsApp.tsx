import { useEffect, useState } from "react";
import { api } from "../../api";
import { installCommands } from "../../commands";
import { handleSettingsKeydown } from "../../commands/settingsWindow";
import { MenuHost } from "../../components/menu";
import { SettingsWindow } from "../../components/settings/SettingsWindow";
import { Toasts } from "../../components/Toasts";
import { disposeAll } from "../../lib/disposeAll";
import { markSettingsWindow } from "../../lib/settingsWindow";
import { useErrorToasts } from "../../lib/useErrorToasts";
import { useFollowSettings } from "../../lib/useFollowSettings";
import { trackWindowFocus } from "../../lib/windowFocus";
import { QueryRoot } from "../../queries/QueryRoot";

/** The page Rust opened the window at (`?page=`); later pages arrive as `open_settings`. */
function initialPage(): string | undefined {
  return new URLSearchParams(window.location.search).get("page") ?? undefined;
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
  const [page, setPage] = useState(initialPage);
  // Re-installing applies the user's shortcut overrides, so Shortcuts lists every command as bound.
  useFollowSettings(installCommands);
  useWindowSetup();
  useErrorToasts();
  useEffect(() => {
    const subscriptions = [api.onOpenSettings(setPage)];
    return () => disposeAll(subscriptions);
  }, []);

  return (
    <>
      <SettingsWindow page={page} onPage={setPage} />
      <MenuHost />
      <Toasts />
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
