import { useLayoutEffect, useState, type CSSProperties } from "react";
import { useStore } from "../../state/store";
import { Modal } from "../Modal";
import { Group, Row } from "../settings/controls";
import { TimelineControls } from "../settings/TimelineControls";

const GAP = 6;
const EDGE = 8;

/** Under the header's gear, right-aligned to it; centered when the gear isn't there. */
function useAnchoredStyle(): CSSProperties | undefined {
  const [style, setStyle] = useState<CSSProperties>();
  useLayoutEffect(() => {
    const rect = document.querySelector("[data-timeline-settings]")?.getBoundingClientRect();
    if (rect) setStyle({ position: "fixed", top: rect.bottom + GAP, right: Math.max(EDGE, window.innerWidth - rect.right) });
  }, []);
  return style;
}

/** The Day header's timeline settings: the controls of Settings → Timeline and the hidden apps. */
export function TimelineSettingsPopover() {
  const close = useStore((s) => s.closeDialog);
  const settings = useStore((s) => s.settings);
  const hiddenApps = useStore((s) => s.dayView?.hiddenApps ?? []);
  const style = useAnchoredStyle();
  if (!settings) return null;
  const unhide = (appKey: string) => void useStore.getState().setAppSetting(appKey, { hidden: false });
  return (
    <Modal onClose={close} variant="panel" className="timeline-popover" label="Timeline settings" style={style}>
      <Group>
        <TimelineControls timeline={settings.timeline} />
      </Group>
      <Group title={`Hidden apps: ${hiddenApps.length}`}>
        {hiddenApps.length === 0 && <Row label="No hidden apps" description="A block's menu hides its app." />}
        {hiddenApps.map((app) => (
          <Row key={app.appKey} label={app.app}>
            <button type="button" className="btn" onClick={() => unhide(app.appKey)}>
              Unhide
            </button>
          </Row>
        ))}
      </Group>
    </Modal>
  );
}
