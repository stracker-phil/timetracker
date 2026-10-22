import type { DayApp, HiddenApp } from "../../api";
import { formatShortDuration } from "../../lib/format";
import { useStore } from "../../state/store";
import { AppIcon } from "../AppIcon";
import { Modal } from "../Modal";
import { Group, Row, Segmented, Switch } from "../settings/controls";
import { TITLES, saveTimeline } from "../settings/timelineActions";
import { useAnchoredStyle } from "../useAnchoredStyle";

const NO_APPS: DayApp[] = [];
const NO_HIDDEN: HiddenApp[] = [];

function AppLaneRow({ app }: { app: DayApp }) {
  const toggle = (ownLane: boolean) => void useStore.getState().setAppSetting(app.appKey, { ownLane });
  return (
    <Row label={app.app} icon={<AppIcon app={app.app} bundleId={app.bundleId} size={16} />} description={formatShortDuration(app.activeMs)}>
      <input className="switch" type="checkbox" role="switch" aria-label={`${app.app} lane`} checked={app.ownLane} onChange={(e) => toggle(e.target.checked)} />
    </Row>
  );
}

function HiddenApps({ apps }: { apps: HiddenApp[] }) {
  const unhide = (appKey: string) => void useStore.getState().setAppSetting(appKey, { hidden: false });
  return (
    <Group title={apps.length > 0 ? `Hidden apps (${apps.length})` : "Hidden apps"}>
      {apps.length === 0 && <Row label="No hidden apps" description="A block's menu hides its app." />}
      {apps.map((app) => (
        <Row key={app.appKey} label={app.app}>
          <button type="button" className="btn" onClick={() => unhide(app.appKey)}>
            Unhide
          </button>
        </Row>
      ))}
    </Group>
  );
}

/**
 * The Day toolbar's View options, under its button: the summary and activity lanes, Compact and Titles, a lane
 * of its own per app of the day, and the hidden apps. The other timeline settings live in Settings → Timeline.
 */
export function ViewOptionsPopover() {
  const close = useStore((s) => s.closeDialog);
  const timeline = useStore((s) => s.settings?.timeline);
  const apps = useStore((s) => s.dayView?.apps ?? NO_APPS);
  const hiddenApps = useStore((s) => s.dayView?.hiddenApps ?? NO_HIDDEN);
  const style = useAnchoredStyle("[data-view-options]", "right");
  if (!timeline) return null;
  return (
    <Modal onClose={close} variant="popover" className="view-options" label="View options" style={style}>
      <Group title="Lanes">
        <Switch label="Summary" description={`The main app of every ${timeline.summaryMinutes} minutes.`} checked={timeline.summaryLane} onChange={(summaryLane) => saveTimeline({ summaryLane })} />
        <Switch label="Activity" description="Everything tracked, app by app." checked={timeline.showAllActivity} onChange={(showAllActivity) => saveTimeline({ showAllActivity })} />
      </Group>
      <Group title="Blocks">
        <Switch label="Compact" description="Shorter rows, one line of text per block." checked={timeline.compact} onChange={(compact) => saveTimeline({ compact })} />
        <Segmented label="Titles" description="The document or page name, or the app." options={TITLES} value={timeline.titles} onChange={(titles) => saveTimeline({ titles })} />
      </Group>
      <Group title="App lanes">
        {apps.length === 0 && <Row label="No apps on this day" />}
        {apps.map((app) => (
          <AppLaneRow key={app.appKey} app={app} />
        ))}
      </Group>
      <HiddenApps apps={hiddenApps} />
    </Modal>
  );
}
