import type { DayApp, TimelineSettings } from "../../api";
import { formatShortDuration } from "../../lib/format";
import { useStore } from "../../state/store";
import { AppIcon } from "../AppIcon";
import { Modal } from "../Modal";
import { Group, Row, Switch } from "../settings/controls";
import { useAnchoredStyle } from "../useAnchoredStyle";

const NO_APPS: DayApp[] = [];

const saveTimeline = (patch: Partial<TimelineSettings>) => void useStore.getState().saveTimeline(patch);

function AppLaneRow({ app }: { app: DayApp }) {
  const toggle = (ownLane: boolean) => void useStore.getState().setAppSetting(app.appKey, { ownLane });
  return (
    <Row label={app.app} icon={<AppIcon app={app.app} bundleId={app.bundleId} size={16} />} description={formatShortDuration(app.activeMs)}>
      <input className="switch" type="checkbox" role="switch" aria-label={`${app.app} lane`} checked={app.ownLane} onChange={(e) => toggle(e.target.checked)} />
    </Row>
  );
}

/** The Day header's Lanes: the summary lane, the activity lane, and a lane of its own for each of the day's apps. */
export function LanesPopover() {
  const close = useStore((s) => s.closeDialog);
  const timeline = useStore((s) => s.settings?.timeline);
  const apps = useStore((s) => s.dayView?.apps ?? NO_APPS);
  const style = useAnchoredStyle("[data-lanes-button]", "right");
  if (!timeline) return null;
  return (
    <Modal onClose={close} variant="popover" className="lanes-popover" label="Lanes" style={style}>
      <Group>
        <Switch label="Summary" description={`The main app of every ${timeline.summaryMinutes} minutes.`} checked={timeline.summaryLane} onChange={(summaryLane) => saveTimeline({ summaryLane })} />
        <Switch label="Activity" description="Everything tracked, app by app." checked={timeline.showAllActivity} onChange={(showAllActivity) => saveTimeline({ showAllActivity })} />
      </Group>
      <Group title="App lanes">
        {apps.length === 0 && <Row label="No apps on this day" />}
        {apps.map((app) => (
          <AppLaneRow key={app.appKey} app={app} />
        ))}
      </Group>
    </Modal>
  );
}
