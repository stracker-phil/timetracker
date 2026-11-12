import { type AppOverride, type AppSettingPatch, type KnownApp } from "../../api";
import { formatDuration } from "../../lib/format";
import type { SettingsPageProps, SettingsRow } from "../../registry/settingsPages";
import { useStore } from "../../state/store";
import { AppIcon } from "../AppIcon";
import { Group, Row } from "./controls";
import { TimelineControls } from "./TimelineControls";
import { useApps } from "../../queries/system";

function AppSwitch({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="inline-switch">
      <input className="switch" type="checkbox" role="switch" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

/** Tracked time, or a note while it is under a minute. */
function trackedLabel(totalMs: number) {
  if (totalMs >= 60_000) return <span className="tabular">{formatDuration(totalMs)} tracked</span>;
  return totalMs > 0 ? "Under a minute tracked" : "Not tracked yet";
}

function AppRow({ app, override }: { app: KnownApp; override: AppOverride | undefined }) {
  const set = (patch: AppSettingPatch) => void useStore.getState().setAppSetting(app.appKey, patch);
  return (
    <Row label={app.app} icon={<AppIcon app={app.app} bundleId={app.appKey} />} description={trackedLabel(app.totalMs)}>
      <div className="app-controls">
        <AppSwitch label="Hidden" checked={!!override?.hidden} onChange={(hidden) => set({ hidden })} />
        <AppSwitch label="Own lane" checked={!!override?.ownLane} onChange={(ownLane) => set({ ownLane })} />
      </div>
    </Row>
  );
}

export const TIMELINE_ROWS: SettingsRow[] = [
  { id: "summary-lane", title: "Summary lane", keywords: ["lanes", "main app"] },
  { id: "summary-interval", title: "Summary interval", keywords: ["lanes", "minutes"], under: "summary-lane" },
  { id: "show-all-activity", title: "Show all activity", keywords: ["lanes", "activity lane"] },
  { id: "compact-activity", title: "Compact activity", keywords: ["dense", "rows"] },
  { id: "group-by-app", title: "Group by app and website", keywords: ["merge", "windows"] },
  { id: "join-short-gaps", title: "Join short gaps", keywords: ["continuity", "merge"] },
  { id: "hide-short-activity", title: "Hide activity shorter than", keywords: ["noise", "short"] },
  { id: "titles", title: "Titles", keywords: ["labels", "document", "page name"] },
  { id: "time-snapping", title: "Time snapping (minutes)", keywords: ["snap", "grid"] },
  { id: "apps", title: "Apps", keywords: ["hidden", "own lane", "hide an app"] },
];

export function TimelinePage({ s }: SettingsPageProps) {
  const { data: apps, error } = useApps();
  return (
    <>
      <Group>
        <TimelineControls timeline={s.timeline} />
      </Group>
      <Group title="Apps" anchor="apps">
        {error && <Row label="Could not list the apps" description={error.message} />}
        {apps?.length === 0 && <Row label="No apps tracked yet" />}
        {apps?.map((app) => <AppRow key={app.appKey} app={app} override={s.apps[app.appKey]} />)}
      </Group>
    </>
  );
}
