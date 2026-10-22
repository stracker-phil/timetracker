/** Settings → Capture and Tracker: what the tracker captures, what it never records, and when time counts as away. */
import { Plus, Search, X } from "lucide-react";
import { useState } from "react";
import { api, SETTINGS_LIMITS, type AppSeen, type CaptureSettings } from "../../api";
import { formatDuration } from "../../lib/format";
import { useApiResult } from "../../lib/useApiResult";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { useStore } from "../../state/store";
import { InsetField } from "../Field";
import { Modal } from "../Modal";
import { Group, NumberField, Row, Switch } from "./controls";

/** Look-back window, in days, for the app picker. */
const RECENT_DAYS = 30;

const CLI_EXAMPLE = 'timetracker activity add --source planner --title "Weekly sync" --start 10:00 --end 10:45';

/** How an app is stored in `excludedApps`: its bundle id, or its name without one, in lower case. */
const appId = (a: AppSeen) => (a.bundleId ?? a.app).toLowerCase();

function RemoveButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button type="button" className="icon-btn" aria-label={label} title={label} onClick={onClick}>
      <X size="1.077rem" />
    </button>
  );
}

function AppPicker({ apps, excluded, onPick, onClose }: { apps: AppSeen[]; excluded: string[]; onPick: (id: string) => void; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const choices = apps.filter((a) => {
    const id = appId(a);
    return !excluded.includes(id) && (a.app.toLowerCase().includes(needle) || id.includes(needle));
  });
  return (
    <Modal onClose={onClose} labelledBy="app-picker-title" className="app-picker">
      <h2 id="app-picker-title">Exclude an app</h2>
      <InsetField icon={<Search size="1.077rem" />}>
        <input autoFocus aria-label="Search apps" placeholder="Search" value={query} onChange={(e) => setQuery(e.target.value)} />
      </InsetField>
      {choices.length === 0 && <p className="hint">{apps.length === 0 ? `No apps recorded in the last ${RECENT_DAYS} days.` : "No app matches."}</p>}
      <ul className="app-picker-list">
        {choices.map((a) => (
          <li key={appId(a)}>
            <button type="button" className="app-picker-row" onClick={() => onPick(appId(a))}>
              <span className="grow-text">{a.app}</span>
              <span className="hint tabular">{formatDuration(a.totalMs)}</span>
            </button>
          </li>
        ))}
      </ul>
    </Modal>
  );
}

function ExcludedApps({ capture, setCapture }: { capture: CaptureSettings; setCapture: (patch: Partial<CaptureSettings>) => void }) {
  const [picking, setPicking] = useState(false);
  const { data: apps } = useApiResult(`apps-seen:${RECENT_DAYS}`, () => api.appsSeen(RECENT_DAYS));
  const nameOf = (id: string) => apps?.find((a) => appId(a) === id)?.app ?? id;
  const remove = (id: string) => setCapture({ excludedApps: capture.excludedApps.filter((a) => a !== id) });

  return (
    <Group title="Excluded apps">
      {capture.excludedApps.map((id) => (
        <Row key={id} label={nameOf(id)} description={<span className="mono">{id}</span>}>
          <RemoveButton label={`Remove ${nameOf(id)}`} onClick={() => remove(id)} />
        </Row>
      ))}
      <Row label={capture.excludedApps.length === 0 ? "No excluded apps" : "Another app"} description="While an excluded app is in front, nothing is recorded; the time stays a gap.">
        <button type="button" className="btn" onClick={() => setPicking(true)}>
          <Plus size="1.077rem" /> Add app…
        </button>
      </Row>
      {picking && (
        <AppPicker
          apps={apps ?? []}
          excluded={capture.excludedApps}
          onClose={() => setPicking(false)}
          onPick={(id) => {
            setPicking(false);
            setCapture({ excludedApps: [...capture.excludedApps, id] });
          }}
        />
      )}
    </Group>
  );
}

function ExcludedSites({ capture, setCapture }: { capture: CaptureSettings; setCapture: (patch: Partial<CaptureSettings>) => void }) {
  const [draft, setDraft] = useState("");
  const [error, setError] = useState("");
  const remove = (domain: string) => setCapture({ excludedDomains: capture.excludedDomains.filter((d) => d !== domain) });
  const add = () => {
    const domain = draft.trim().toLowerCase();
    if (!domain) return setError("Enter a domain, like bank.example.com.");
    setError("");
    setDraft("");
    // Rust reduces a pasted address to its host and drops repeats.
    setCapture({ excludedDomains: [...capture.excludedDomains, domain] });
  };
  return (
    <Group title="Excluded websites">
      {capture.excludedDomains.map((domain) => (
        <Row key={domain} label={domain}>
          <RemoveButton label={`Remove ${domain}`} onClick={() => remove(domain)} />
        </Row>
      ))}
      <Row label="Add a website" description={error ? <span role="alert">{error}</span> : "The domain and every subdomain of it, in every browser."}>
        <InsetField icon={<Plus size="1.077rem" />}>
          <input
            aria-label="Website to exclude"
            placeholder="bank.example.com"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") add();
            }}
          />
        </InsetField>
      </Row>
    </Group>
  );
}

/** Read-only: other apps add activity through the CLI or `timetracker://` links, not here. */
function ExternalSources() {
  const dataVersion = useStore((s) => s.dataVersion);
  const { data: sources } = useApiResult(`activity-sources:${dataVersion}`, () => api.activitySources(), { keepData: true });
  return (
    <Group title="External sources">
      {sources?.map((name) => (
        <Row key={name} label={name} />
      ))}
      <Row
        stacked
        label={sources?.length ? "Add another" : "No external sources yet"}
        description="Other apps, such as a planner, add their activity with the timetracker command. Each source gets its own lane next to the tracker's."
      >
        <code className="mono cli-example">{CLI_EXAMPLE}</code>
      </Row>
    </Group>
  );
}

export function CapturePage({ s, set }: SettingsPageProps) {
  const capture = s.capture;
  const setCapture = (patch: Partial<CaptureSettings>) => set({ capture: { ...capture, ...patch } });
  return (
    <>
      <Group title="Recorded">
        <Switch
          label="Capture page addresses in browsers"
          description="macOS asks once per browser whether TimeTracker may read its address bar. Firefox cannot be read; only its window titles are recorded."
          checked={capture.captureUrls}
          onChange={(captureUrls) => setCapture({ captureUrls })}
        />
        <Switch
          label="Capture document paths"
          description="The file open in the front window, for apps that report it, such as TextEdit, Pages, Preview and Xcode."
          checked={capture.captureDocuments}
          onChange={(captureDocuments) => setCapture({ captureDocuments })}
        />
      </Group>
      <ExcludedApps capture={capture} setCapture={setCapture} />
      <ExcludedSites capture={capture} setCapture={setCapture} />
      <ExternalSources />
    </>
  );
}

const AWAY_PROMPT_MINUTES = [5, 10, 15, 20, 30, 45, 60];

/** The fixed choices, plus a hand-edited value so the select still shows it. */
const awayPromptChoices = (current: number) =>
  AWAY_PROMPT_MINUTES.includes(current) ? AWAY_PROMPT_MINUTES : [...AWAY_PROMPT_MINUTES, current].sort((a, b) => a - b);

export function TrackerPage({ s, set }: SettingsPageProps) {
  const tracker = s.tracker;
  return (
    <Group>
      <NumberField
        label="Away after"
        description="Minutes without keyboard or mouse input before the time counts as away."
        suffix="min"
        value={tracker.idleMinutes}
        min={SETTINGS_LIMITS.idleMinutes.min}
        max={SETTINGS_LIMITS.idleMinutes.max}
        onCommit={(idleMinutes) => set({ tracker: { ...tracker, idleMinutes } })}
      />
      <Switch
        label="Ask what I did after being away"
        description="After a long away time, a small window asks for the project to record it for. Pauses never ask."
        checked={tracker.awayPromptEnabled}
        onChange={(awayPromptEnabled) => set({ tracker: { ...tracker, awayPromptEnabled } })}
      />
      <Row label="For longer than" htmlFor="settings-away-prompt-minutes" nested>
        <div className="settings-input">
          <select
            id="settings-away-prompt-minutes"
            className="input number"
            disabled={!tracker.awayPromptEnabled}
            value={tracker.awayPromptMinutes}
            onChange={(e) => set({ tracker: { ...tracker, awayPromptMinutes: Number(e.target.value) } })}
          >
            {awayPromptChoices(tracker.awayPromptMinutes).map((minutes) => (
              <option key={minutes} value={minutes}>
                {minutes}
              </option>
            ))}
          </select>
          <span className="muted">min</span>
        </div>
      </Row>
    </Group>
  );
}
