/** Settings → Status: what this build is, what it captures and where it keeps its files. */
import { toAppError, type CaptureKind, type CommandLine, type PermissionStatus } from "../../api";
import type { SettingsRow } from "../../registry/settingsPages";
import { Group, Row } from "./controls";
import { useTracker } from "../../queries/status";
import { useAppStatus } from "../../queries/system";

export const STATUS_ROWS: SettingsRow[] = [
  { id: "status-build", title: "Build", keywords: ["debug", "release", "version"] },
  { id: "status-capture", title: "Capture", keywords: ["demo", "real windows"] },
  { id: "status-accessibility", title: "Accessibility", keywords: ["permission", "privacy"] },
  { id: "status-tracker", title: "Tracker", keywords: ["running", "paused", "away"] },
  { id: "status-settings-folder", title: "Settings folder", keywords: ["config", "keybindings", "socket"] },
  { id: "status-database", title: "Database", keywords: ["data file", "SQLite"] },
  { id: "status-log-folder", title: "Log folder", keywords: ["logs"] },
  { id: "status-command", title: "Command", keywords: ["CLI", "command line", "timetracker", "PATH"] },
];

const CAPTURE_TEXT: Record<CaptureKind, string> = { real: "The windows in front", demo: "Made-up windows (debug build)" };

const PERMISSION_TEXT: Record<PermissionStatus, string> = { granted: "Allowed", denied: "Not allowed", unknown: "Not asked yet" };

function commandText({ link, path }: CommandLine): string {
  switch (link) {
    case "installed":
      return `Installed at ${path}`;
    case "notInstalled":
      return `Not installed (${path})`;
    case "notApplicable":
      return "Not linked for debug builds";
    default:
      return link satisfies never;
  }
}

/** Machine text: mono, selectable, wrapping anywhere. */
function Path({ children }: { children: string }) {
  return <span className="settings-path mono selectable">{children}</span>;
}

export function StatusPage() {
  const { data, error } = useAppStatus();
  const tracker = useTracker();
  if (error && !data) {
    return (
      <div className="settings-alert selectable" role="alert">
        Couldn't read the status: {toAppError(error).message}
      </div>
    );
  }
  if (!data) return null;
  return (
    <>
      <Group>
        <Row label="Build" anchor="status-build">
          <span className="tabular selectable">
            {data.debug ? "Debug" : "Release"} {data.version}
          </span>
        </Row>
      </Group>
      <Group title="Tracking">
        <Row label="Capture" anchor="status-capture">
          {CAPTURE_TEXT[data.capture]}
        </Row>
        <Row label="Accessibility" anchor="status-accessibility" description="Lets TimeTracker read the front window's title.">
          {tracker ? PERMISSION_TEXT[tracker.permission] : "Unknown"}
        </Row>
        <Row label="Tracker" anchor="status-tracker">
          {tracker?.text ?? "Off: the data store isn't open"}
        </Row>
      </Group>
      <Group title="Files">
        <Row label="Settings folder" anchor="status-settings-folder" stacked>
          <Path>{data.settingsFolder}</Path>
        </Row>
        <Row label="Database" anchor="status-database" stacked>
          <Path>{data.database}</Path>
        </Row>
        <Row label="Log folder" anchor="status-log-folder" stacked>
          <Path>{data.logFolder}</Path>
        </Row>
        <Row label="Command" anchor="status-command" description="Lets the shell and other apps reach TimeTracker." stacked>
          <Path>{commandText(data.commandLine)}</Path>
        </Row>
      </Group>
    </>
  );
}
