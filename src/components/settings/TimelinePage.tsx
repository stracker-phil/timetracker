import { api } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { useStore } from "../../state/store";
import { Group, Row } from "./controls";

const NAME_DAYS = 365;

/** Apps hidden from the timeline (its block menu hides them), each with Show. */
export function TimelinePage({ s }: SettingsPageProps) {
  const hidden = Object.entries(s.apps)
    .filter(([, app]) => app.hidden)
    .map(([bundleId]) => bundleId)
    .sort();
  const { data: apps } = useApiResult(hidden.length > 0 ? `apps-seen:${NAME_DAYS}` : null, () => api.appsSeen(NAME_DAYS));
  const nameOf = (bundleId: string) => apps?.find((a) => a.bundleId === bundleId)?.app ?? bundleId;
  const show = (bundleId: string) => void useStore.getState().setAppSetting(bundleId, { hidden: false });

  return (
    <Group title="Hidden apps">
      {hidden.length === 0 && <Row label="No hidden apps" description="Right-click a block on the timeline to hide its app." />}
      {hidden.map((bundleId) => (
        <Row key={bundleId} label={nameOf(bundleId)} description={<span className="mono">{bundleId}</span>}>
          <button type="button" className="btn" onClick={() => show(bundleId)}>
            Show
          </button>
        </Row>
      ))}
    </Group>
  );
}
