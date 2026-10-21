import { useEffect } from "react";
import { api, toAppError, type LaunchEvent } from "./api";
import { ActivityDebug } from "./components/ActivityDebug";
import { CommandPalette } from "./components/CommandPalette";
import { Dialogs } from "./components/Dialogs";
import { EmptyView } from "./components/EmptyView";
import { PermissionDialog } from "./components/PermissionDialog";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { Toasts } from "./components/Toasts";
import { Toolbar } from "./components/Toolbar";
import { handleKeydown, installCommands, runMenuCommand } from "./commands";
import { answerRemote } from "./commands/remote";
import { applyAppearance, useSystemDark } from "./lib/theme";
import { flushUiState, useStore } from "./state/store";
import "./styles/fonts.css";
import "./styles/schemes/ayu-mirage.css";
import "./styles/schemes/monokai.css";
import "./styles/schemes/nord.css";
import "./styles/schemes/solarized-dark.css";
import "./styles/schemes/ayu-light.css";
import "./styles/schemes/catppuccin-latte.css";
import "./styles/schemes/github-light.css";
import "./styles/schemes/solarized-light.css";
import "./styles/theme.css";
import "./styles/app.css";

function handleLaunchEvents(events: LaunchEvent[]) {
  const { toast } = useStore.getState();
  for (const event of events) {
    if (event.type === "error") toast("error", event.message);
  }
}

function useBackendEvents() {
  useEffect(() => {
    const { loadSettings, loadUiState, loadTracker, setTracker, toast, bumpData } = useStore.getState();
    const drainLaunches = () => api.takeLaunchEvents().then(handleLaunchEvents, (e) => toast("error", `Launch request failed: ${toAppError(e).message}`));

    // Re-installing applies the user's keybinding overrides, which arrive with the settings.
    void loadSettings().then(() => (installCommands(), drainLaunches()));
    void loadUiState();
    void loadTracker();
    const unlisten = [
      api.onLaunch(() => void drainLaunches()),
      api.onCommand(runMenuCommand),
      api.onRemote((e) => void answerRemote(e)),
      api.onDataChanged(bumpData),
      api.onTrackerStatus(setTracker),
    ];

    const onError = (e: ErrorEvent) => toast("error", `Unexpected error: ${e.message}`);
    const onRejection = (e: PromiseRejectionEvent) => toast("error", `Unexpected error: ${toAppError(e.reason).message}`);
    window.addEventListener("keydown", handleKeydown, true);
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    window.addEventListener("beforeunload", flushUiState);
    return () => {
      unlisten.forEach((p) => void p.then((fn) => fn()).catch(() => {}));
      window.removeEventListener("keydown", handleKeydown, true);
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
      window.removeEventListener("beforeunload", flushUiState);
    };
  }, []);
}

/** Hiding the sidebar removes it entirely; `main` keeps its tree position so views keep their state. */
function Workspace() {
  const sidebarHidden = useStore((s) => s.uiState.sidebarHidden);
  const view = useStore((s) => s.view);
  return (
    <div className={sidebarHidden ? "workspace no-sidebar" : "workspace"}>
      {!sidebarHidden && <Sidebar />}
      <main className="main">
        {view === "day" ? <ActivityDebug /> : <EmptyView />}
      </main>
    </div>
  );
}

export default function App() {
  const settings = useStore((s) => s.settings);
  const systemDark = useSystemDark();
  useBackendEvents();

  useEffect(() => {
    if (settings) applyAppearance(settings, systemDark);
  }, [settings, systemDark]);

  return (
    <>
      <div className="app">
        <Toolbar />
        <Workspace />
        <StatusBar />
      </div>
      <Dialogs />
      <PermissionDialog />
      <CommandPalette />
      <Toasts />
    </>
  );
}
