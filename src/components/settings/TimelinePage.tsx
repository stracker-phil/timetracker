import { api, type AppOverride, type AppSettingPatch, type KnownApp } from "../../api";
import { formatHm } from "../../lib/format";
import { useApiResult } from "../../lib/useApiResult";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { useStore } from "../../state/store";
import { SegmentedControl } from "../SegmentedControl";
import { Group, Row } from "./controls";
import { TimelineControls } from "./TimelineControls";

const GROUPING = ["default", "on", "off"] as const;
const TITLES = ["default", "document", "app"] as const;

function groupingOf(app: AppOverride | undefined): (typeof GROUPING)[number] {
  if (app?.groupByApp == null) return "default";
  return app.groupByApp ? "on" : "off";
}

function AppSwitch({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="inline-switch">
      <input className="switch" type="checkbox" role="switch" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

function AppRow({ app, override }: { app: KnownApp; override: AppOverride | undefined }) {
  const set = (patch: AppSettingPatch) => void useStore.getState().setAppSetting(app.appKey, patch);
  return (
    <Row label={app.app} description={<span className="mono">{app.totalMs > 0 ? formatHm(app.totalMs) : app.appKey}</span>} stacked>
      <div className="app-controls">
        <span className="o3">Group by app</span>
        <SegmentedControl
          label={`Group ${app.app} by app`}
          options={GROUPING}
          value={groupingOf(override)}
          onChange={(v) => set({ groupByApp: v === "default" ? null : v === "on" })}
        />
        <span className="o3">Titles</span>
        <SegmentedControl
          label={`Titles of ${app.app}`}
          options={TITLES}
          value={override?.titles ?? "default"}
          onChange={(v) => set({ titles: v === "default" ? null : v })}
        />
        <AppSwitch label="Hidden" checked={!!override?.hidden} onChange={(hidden) => set({ hidden })} />
        <AppSwitch label="Own lane" checked={!!override?.ownLane} onChange={(ownLane) => set({ ownLane })} />
      </div>
    </Row>
  );
}

export function TimelinePage({ s }: SettingsPageProps) {
  const { data: apps, error } = useApiResult("apps", api.appsList);
  return (
    <>
      <Group>
        <TimelineControls timeline={s.timeline} />
      </Group>
      <Group title="Apps">
        {error && <Row label="Could not list the apps" description={error.message} />}
        {apps?.length === 0 && <Row label="No apps tracked yet" />}
        {apps?.map((app) => <AppRow key={app.appKey} app={app} override={s.apps[app.appKey]} />)}
      </Group>
    </>
  );
}
