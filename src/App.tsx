import { useEffect } from "react";
import { api, toAppError, type LaunchEvent } from "./api";
import { CommandPalette } from "./components/CommandPalette";
import { DayPage } from "./components/day/DayPage";
import { Dialogs } from "./components/Dialogs";
import { EmptyView } from "./components/EmptyView";
import { Welcome } from "./components/onboarding/Welcome";
import { PermissionDialog } from "./components/PermissionDialog";
import { ProjectsPage } from "./components/projects/ProjectsPage";
import { ReportsView } from "./components/reports/ReportsView";
import { RulesView } from "./components/rules/RulesView";
import { MonthView } from "./components/calendar/MonthView";
import { WeekView } from "./components/calendar/WeekView";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { Toasts } from "./components/Toasts";
import { Toolbar } from "./components/Toolbar";
import { handleKeydown, installCommands, runMenuCommand } from "./commands";
import { installMenuSync } from "./commands/menu";
import { answerRemote } from "./commands/remote";
import { applyAppearance, useSystemDark } from "./lib/theme";
import { flushUiState, useStore, type View } from "./state/store";
import "./styles";

function handleLaunchEvents(events: LaunchEvent[]) {
  const { toast, goToDay } = useStore.getState();
  for (const event of events) {
    if (event.type === "error") toast("error", event.message);
    if (event.type === "notice") toast("info", event.message);
    if (event.type === "showDay") goToDay(event.day);
  }
}

function useBackendEvents() {
  useEffect(() => {
    const { loadSettings, loadUiState, restoreDay, loadTracker, loadProjects, setTracker, toast, dataChanged } = useStore.getState();
    const drainLaunches = () => api.takeLaunchEvents().then(handleLaunchEvents, (e) => toast("error", `Launch request failed: ${toAppError(e).message}`));

    // Re-installing applies the user's keybinding overrides, which arrive with the settings.
    void loadSettings().then(() => (installCommands(), drainLaunches()));
    void loadUiState().then(restoreDay);
    void loadTracker();
    void loadProjects();
    void useStore.getState().loadSavedReports();
    void useStore.getState().loadTimer();
    const unlisten = [
      api.onTimerStatus(useStore.getState().setTimerStatus),
      api.onLaunch(() => void drainLaunches()),
      api.onCommand(runMenuCommand),
      api.onRemote((e) => void answerRemote(e)),
      api.onDataChanged(dataChanged),
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

function useMenuSync() {
  useEffect(() => installMenuSync(), []);
}

function ViewContent({ view }: { view: View }) {
  switch (view) {
    case "day":
      return <DayPage />;
    case "projects":
      return <ProjectsPage />;
    case "reports":
      return <ReportsView />;
    case "rules":
      return <RulesView />;
    case "week":
      return <WeekView />;
    case "month":
      return <MonthView />;
    default:
      return <EmptyView />;
  }
}

/** Hiding the sidebar removes it entirely; `main` keeps its tree position so views keep their state. */
function Workspace() {
  const sidebarHidden = useStore((s) => s.uiState.sidebarHidden);
  const view = useStore((s) => s.view);
  return (
    <div className={sidebarHidden ? "workspace no-sidebar" : "workspace"}>
      {!sidebarHidden && <Sidebar />}
      <main className="main">
        <ViewContent view={view} />
      </main>
    </div>
  );
}

export default function App() {
  const settings = useStore((s) => s.settings);
  const systemDark = useSystemDark();
  useBackendEvents();
  useMenuSync();

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
      <Welcome />
      <CommandPalette />
      <Toasts />
    </>
  );
}
