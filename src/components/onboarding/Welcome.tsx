import { useState } from "react";
import { api, type TrackerStatus } from "../../api";
import { bindingLabel } from "../../commands";
import { useApiResult } from "../../lib/useApiResult";
import { useStore } from "../../state/store";
import { KeyCap } from "../Field";
import { Modal } from "../Modal";
import { LaunchAtLoginSwitch } from "../settings/LaunchAtLoginSwitch";

const PANELS = 3;

function RecordedPanel() {
  const { data: info } = useApiResult("welcome-data-info", api.dataInfo);
  return (
    <>
      <h2 id="welcome-title">Welcome to TimeTracker</h2>
      <p>
        TimeTracker records which app is in front, its window title, the URL or document it shows, and when you are away. Your day then
        shows what you worked on, ready to turn into recordings for your projects.
      </p>
      <p className="muted">Never recorded: keystrokes, screen contents, screenshots, audio or the clipboard. Nothing leaves this Mac.</p>
      {info && (
        <p>
          Everything stays in the data folder: <span className="mono selectable">{info.dir}</span>
        </p>
      )}
    </>
  );
}

function permissionLine(tracker: TrackerStatus | null): string {
  if (!tracker) return "The tracker is off: the data store could not be opened (Settings → Data).";
  return tracker.permission === "granted" ? "Access granted. Window titles are recorded." : "Not granted yet.";
}

function AccessibilityPanel({ onSkip }: { onSkip: () => void }) {
  const tracker = useStore((s) => s.tracker);
  const requestPermission = useStore((s) => s.requestPermission);
  return (
    <>
      <h2 id="welcome-title">Allow reading window titles</h2>
      <p>
        macOS shares window titles only with apps you allow under Privacy &amp; Security → Accessibility. Without it, TimeTracker sees only
        which app is in front.
      </p>
      <p role="status">{permissionLine(tracker)}</p>
      <div className="actions">
        <button type="button" className="btn" onClick={onSkip}>
          Skip for now
        </button>
        <button type="button" className="btn" onClick={() => void requestPermission()}>
          Grant access
        </button>
      </div>
    </>
  );
}

function StartPanel() {
  const shortcut = useStore((s) => s.settings?.timerShortcut ?? null);
  return (
    <>
      <h2 id="welcome-title">Always at hand</h2>
      <p>TimeTracker lives in the menu bar; closing the window keeps it recording.</p>
      <div className="settings-card">
        <LaunchAtLoginSwitch />
      </div>
      {shortcut && (
        <p>
          Press <KeyCap keys={bindingLabel(shortcut)} /> in any app to start a timer. Change it in Settings → Shortcuts.
        </p>
      )}
    </>
  );
}

/** The first-run sheet over the Day view, shown until Done (or Escape); Settings → General shows it again. */
export function Welcome() {
  const show = useStore((s) => !s.uiState.onboardingDone);
  // Mounted per showing, so "Show welcome again" starts on the first panel.
  return show ? <WelcomeSheet /> : null;
}

function WelcomeSheet() {
  const granted = useStore((s) => s.tracker?.permission === "granted");
  const finish = useStore((s) => s.finishOnboarding);
  const [panel, setPanel] = useState(0);
  const [skipped, setSkipped] = useState(false);

  const last = panel === PANELS - 1;
  const blocked = panel === 1 && !granted && !skipped;
  const next = () => (last ? void finish() : setPanel(panel + 1));
  const skip = () => {
    setSkipped(true);
    setPanel(2);
  };
  return (
    <Modal onClose={() => void finish()} labelledBy="welcome-title" className="welcome">
      {panel === 0 && <RecordedPanel />}
      {panel === 1 && <AccessibilityPanel onSkip={skip} />}
      {panel === 2 && <StartPanel />}
      <div className="actions">
        <span className="actions-start muted">
          {panel + 1} of {PANELS}
        </span>
        {panel > 0 && (
          <button type="button" className="btn" onClick={() => setPanel(panel - 1)}>
            Back
          </button>
        )}
        <button type="button" className="btn btn-primary" disabled={blocked} onClick={next}>
          {last ? "Done" : "Next"}
        </button>
      </div>
    </Modal>
  );
}
