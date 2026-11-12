import { useEffect } from "react";
import { api, toAppError, type LaunchEvent } from "./api";
import { CommandPalette } from "./components/CommandPalette";
import { DayPage } from "./components/day/DayPage";
import { Dialogs } from "./components/Dialogs";
import { EmptyView } from "./components/EmptyView";
import { MenuHost } from "./components/menu";
import { Welcome } from "./components/onboarding/Welcome";
import { PermissionDialog } from "./components/PermissionDialog";
import { ProjectsPage } from "./components/projects/ProjectsPage";
import { ReportsView } from "./components/reports/ReportsView";
import { RulesView } from "./components/rules/RulesView";
import { TagsPage } from "./components/tags/TagsPage";
import { MonthView } from "./components/calendar/MonthView";
import { WeekView } from "./components/calendar/WeekView";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { Toasts } from "./components/Toasts";
import { TooltipHost } from "./components/TooltipHost";
import { handleKeydown, installCommands, listenPageCommands, runMenuCommand } from "./commands";
import { installMenuSync } from "./commands/menu";
import { answerRemote } from "./commands/remote";
import { disposeAll } from "./lib/disposeAll";
import { applyAppearance, useSystemDark } from "./lib/theme";
import { useErrorToasts } from "./lib/useErrorToasts";
import { queryClient } from "./queries/queryClient";
import { QueryRoot } from "./queries/QueryRoot";
import { projectListsQuery } from "./queries/projects";
import { savedReportsQuery } from "./queries/reports";
import { loadSettings, useSettings } from "./queries/settings";
import { timerQuery, trackerQuery } from "./queries/status";
import { useEvents } from "./queries/useEvents";
import { flushUiState, useStore, type View } from "./state/store";
import "./styles";

function handleLaunchEvents(events: LaunchEvent[]) {
  const { toast, goToDay } = useStore.getState();
  for (const event of events) {
    if (event.type === "error") toast("error", event.message);
    if (event.type === "notice") toast("info", event.message);
    if (event.type === "showDay") goToDay(event.day);
    if (event.type === "command") runMenuCommand(event.id);
  }
}

function useBackendEvents() {
  useEffect(() => {
    const { loadUiState, toast, windowOpened } = useStore.getState();
    const drainLaunches = () => api.takeLaunchEvents().then(handleLaunchEvents, (e) => toast("error", `Launch request failed: ${toAppError(e).message}`));

    // Re-installing applies the user's keybinding overrides, which arrive with the settings.
    void loadSettings().then(() => (installCommands(), drainLaunches()));
    void loadUiState();
    // Read outside components too (commands, toasts), so loaded before any view asks.
    void queryClient.prefetchQuery(trackerQuery);
    void queryClient.prefetchQuery(timerQuery);
    void queryClient.prefetchQuery(projectListsQuery);
    void queryClient.prefetchQuery(savedReportsQuery);
    const subscriptions = [
      api.onLaunch(() => void drainLaunches()),
      api.onWindowOpened(windowOpened),
      listenPageCommands(),
      api.onRemote((e) => void answerRemote(e)),
    ];

    window.addEventListener("keydown", handleKeydown, true);
    window.addEventListener("beforeunload", flushUiState);
    return () => {
      disposeAll(subscriptions);
      window.removeEventListener("keydown", handleKeydown, true);
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
    case "tags":
      return <TagsPage />;
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

function Shell() {
  const settings = useSettings();
  const systemDark = useSystemDark();
  useEvents();
  useBackendEvents();
  useErrorToasts();
  useMenuSync();

  useEffect(() => {
    if (settings) applyAppearance(settings, systemDark);
  }, [settings, systemDark]);

  return (
    <>
      <div className="app">
        <Workspace />
        <StatusBar />
      </div>
      <Dialogs />
      <PermissionDialog />
      <Welcome />
      <CommandPalette />
      <MenuHost />
      <Toasts />
      <TooltipHost />
    </>
  );
}

export default function App() {
  return (
    <QueryRoot>
      <Shell />
    </QueryRoot>
  );
}
