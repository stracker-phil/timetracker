/** Settings → About: the version, and the folders a bug report looks at. */
import { FileText, FolderOpen } from "lucide-react";
import { api } from "../../api";
import type { SettingsRow } from "../../registry/settingsPages";
import { fail } from "../../state/requests";
import { Group, Row } from "./controls";
import { useAppStatus } from "../../queries/system";

export const ABOUT_ROWS: SettingsRow[] = [
  { id: "about-version", title: "Version", keywords: ["release", "build"] },
  { id: "about-data", title: "Data", keywords: ["data folder", "database"] },
  { id: "about-logs", title: "Logs", keywords: ["log folder", "bug report"] },
  { id: "about-readme", title: "README", keywords: ["help", "manual", "documentation"] },
];

/** Runs an app command (Rust's bus), so the button does what the Help menu item does. */
const run = (id: string, failure: string) => () => void api.runCommand(id).catch((e: unknown) => fail(e, failure));

export function AboutPage() {
  const { data } = useAppStatus();
  return (
    <Group>
      <Row label="Version" anchor="about-version">
        <span className="tabular selectable">{data?.version ?? ""}</span>
      </Row>
      <Row label="Data" anchor="about-data" description="Your activity, recordings and projects.">
        <button type="button" className="btn" onClick={run("app:dataFolder", "Couldn't show the data folder")}>
          <FolderOpen size="1.077rem" /> Show Data Folder
        </button>
      </Row>
      <Row label="Logs" anchor="about-logs" description="What TimeTracker did and what went wrong, for a bug report.">
        <button type="button" className="btn" onClick={run("app:logs", "Couldn't open the log folder")}>
          <FolderOpen size="1.077rem" /> Open Log Folder
        </button>
      </Row>
      <Row label="README" anchor="about-readme" description="What TimeTracker does, its keys and its command.">
        <button type="button" className="btn" onClick={run("app:readme", "Couldn't open the README")}>
          <FileText size="1.077rem" /> Open README
        </button>
      </Row>
    </Group>
  );
}
