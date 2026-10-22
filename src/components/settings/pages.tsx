/** The core settings pages. Modules register their own pages from their install function. */
import { Activity, Copy, Database, Download, EyeOff, FolderInput, FolderOpen, GanttChart, Keyboard, Palette, Power, ScrollText } from "lucide-react";
import { useState } from "react";
import { api, BUNDLED_FONTS, LOG_LEVELS, TEXT_SIZES, toAppError, type DataInfo, type LogLevel, type Scheme } from "../../api";
import { copyText } from "../../lib/clipboard";
import { capitalize, formatBytes, formatCount, formatDay } from "../../lib/format";
import { schemeLook, systemIsDark, type Tone } from "../../lib/theme";
import { useApiResult } from "../../lib/useApiResult";
import { registerSettingsPage, type SettingsPageProps } from "../../registry/settingsPages";
import { fail } from "../../state/requests";
import { useStore } from "../../state/store";
import { AccentSwatches } from "../Accent";
import { Modal } from "../Modal";
import { exportBackup, revealFolder, showWelcomeAgain } from "./actions";
import { Group, NumberField, Row, Segmented, Switch, TextField } from "./controls";
import { CapturePage, TrackerPage } from "./CapturePage";
import { LaunchAtLoginSwitch } from "./LaunchAtLoginSwitch";
import { ShortcutsPage } from "./ShortcutsPage";
import { TimelinePage } from "./TimelinePage";

const SCHEME_LABELS: Record<Scheme, string> = { default: "Default", light: "Light", dark: "Dark" };

/** One tile per scheme: a miniature window in the scheme's look. */
function SchemePicker({ label, value, onChange }: { label: string; value: Scheme; onChange: (scheme: Scheme) => void }) {
  return (
    <div className="scheme-picker" role="group" aria-label={label}>
      {(Object.keys(SCHEME_LABELS) as Scheme[]).map((id) => {
        const look = schemeLook(id);
        return (
          <button key={id} type="button" className="scheme-option" aria-pressed={value === id} onClick={() => onChange(id)}>
            <span className="scheme-tile" data-content={look.content} data-sidebar={look.sidebar} aria-hidden="true">
              <span className="scheme-tile-sidebar">
                <span className="scheme-tile-entry" />
              </span>
              <span className="scheme-tile-content">
                <span className="scheme-tile-block" />
              </span>
            </span>
            {SCHEME_LABELS[id]}
          </button>
        );
      })}
    </div>
  );
}

const SCHEME_KEYS = { light: "lightScheme", dark: "darkScheme" } as const;

function AppearancePage({ s, set }: SettingsPageProps) {
  const [mode, setMode] = useState<Tone>(() => (systemIsDark() ? "dark" : "light"));
  const key = SCHEME_KEYS[mode];
  return (
    <>
      <Group title="Look">
        <Segmented label="Mode" description="Pick a scheme for each macOS mode." options={["light", "dark"] as const} value={mode} onChange={setMode} />
        <Row label="Scheme" description={`Used while macOS is in ${mode} mode.`} stacked>
          <SchemePicker label={`${capitalize(mode)} mode scheme`} value={s[key]} onChange={(scheme) => set({ [key]: scheme })} />
        </Row>
        <Row label="Accent" description="Buttons, links, selection and focus.">
          <AccentSwatches value={s.accent} onChange={(accent) => set({ accent })} />
        </Row>
      </Group>
      <Group title="Text">
        <NumberField label="UI size" description="Sidebar, toolbar and labels." suffix="px" value={s.uiFontSize} min={TEXT_SIZES.ui.min} max={TEXT_SIZES.ui.max} onCommit={(uiFontSize) => set({ uiFontSize })} />
        <NumberField label="Data size" description="Times, durations and titles. ⌘= / ⌘- zoom both." suffix="px" value={s.dataFontSize} min={TEXT_SIZES.data.min} max={TEXT_SIZES.data.max} onCommit={(dataFontSize) => set({ dataFontSize })} />
        <TextField label="UI font" description="Any installed family; empty uses the bundled font." placeholder={BUNDLED_FONTS.ui} value={s.fontUi ?? ""} onCommit={(name) => set({ fontUi: name.trim() || null })} />
        <TextField label="Mono font" description="Times and data values. Any installed family; empty uses the bundled font." placeholder={BUNDLED_FONTS.mono} value={s.fontMono ?? ""} onCommit={(name) => set({ fontMono: name.trim() || null })} />
      </Group>
    </>
  );
}

function GeneralPage() {
  return (
    <>
      <Group>
        <LaunchAtLoginSwitch />
      </Group>
      <Group title="Welcome">
        <Row label="Welcome sheet" description="What TimeTracker records, the Accessibility permission and how it starts.">
          <button type="button" className="link-btn" onClick={showWelcomeAgain}>
            Show welcome again
          </button>
        </Row>
      </Group>
    </>
  );
}

function LoggingPage({ s, set }: SettingsPageProps) {
  return (
    <Group>
      <Row label="Log level" htmlFor="settings-log-level" description="Higher levels write more detail.">
        <select id="settings-log-level" className="input" value={s.logLevel} onChange={(e) => set({ logLevel: e.target.value as LogLevel })}>
          {LOG_LEVELS.map((l) => (
            <option key={l} value={l}>{capitalize(l)}</option>
          ))}
        </select>
      </Row>
      <Row label="Log files" description="The three most recent files are kept.">
        <button type="button" className="btn" onClick={() => void api.openLogFolder()}>
          <FolderOpen size="1.077rem" /> Open log folder
        </button>
      </Row>
    </Group>
  );
}

/** Asks before copying the data into `target`; stays open, busy, while the copy runs. */
function MoveSheet({ from, target, moving, onMove, onCancel }: { from: string; target: string; moving: boolean; onMove: () => void; onCancel: () => void }) {
  return (
    <Modal onClose={() => !moving && onCancel()} labelledBy="move-data-title">
      <h2 id="move-data-title">Move the data folder?</h2>
      <p>
        TimeTracker copies its database to <span className="mono selectable">{target}</span> and uses it from then on. Tracking pauses while it copies.
      </p>
      <p className="hint">
        The old files stay in <span className="mono selectable">{from}</span>. Delete them in Finder once you no longer need them.
      </p>
      <div className="actions">
        <button type="button" className="btn" disabled={moving} onClick={onCancel}>
          Cancel
        </button>
        <button type="button" className="btn btn-primary" autoFocus disabled={moving} aria-busy={moving} onClick={onMove}>
          {moving ? "Moving…" : "Move"}
        </button>
      </div>
    </Modal>
  );
}

type MoveOutcome = { kind: "moved" } | { kind: "refused"; reason: string } | { kind: "failed" };

/** Moves the data into `target`; failures other than a refused folder are toasted. */
async function moveData(from: string, target: string): Promise<MoveOutcome> {
  try {
    const moved = await api.dataMove(target);
    const { toast, loadSettings } = useStore.getState();
    toast("success", `Data moved to ${moved.dir}. The old files are still in ${from}.`);
    // The store's copy of the settings still names the old folder.
    void loadSettings();
    return { kind: "moved" };
  } catch (e) {
    const err = toAppError(e);
    if (err.kind === "invalid_input") return { kind: "refused", reason: err.message };
    fail(err, "Moving the data folder failed");
    return { kind: "failed" };
  }
}

/** Location row with Reveal and Move…; a refused target shows under the row. */
function FolderGroup({ info, onMoved }: { info: DataInfo; onMoved: () => void }) {
  const [target, setTarget] = useState<string | null>(null);
  const [moving, setMoving] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  const choose = async () => {
    try {
      const path = await api.chooseFolder();
      if (path) setTarget(path);
    } catch (e) {
      fail(e, "Couldn't choose a folder");
    }
  };

  const move = async (path: string) => {
    setMoving(true);
    const outcome = await moveData(info.dir, path);
    if (outcome.kind === "moved") {
      setRefused(null);
      onMoved();
    } else if (outcome.kind === "refused") {
      setRefused(outcome.reason);
    }
    setMoving(false);
    setTarget(null);
  };

  return (
    <Group title="Folder">
      <Row label="Location" description={<span className="mono selectable">{info.dir}</span>}>
        <button type="button" className="btn" onClick={() => void revealFolder(info.file)}>
          <FolderOpen size="1.077rem" /> Reveal in Finder
        </button>
        <button type="button" className="btn" disabled={!info.available} onClick={() => void choose()}>
          <FolderInput size="1.077rem" /> Move…
        </button>
      </Row>
      {refused && (
        <div className="settings-row">
          <div className="form-field-error selectable" role="alert">
            {refused}
          </div>
        </div>
      )}
      {target && <MoveSheet from={info.dir} target={target} moving={moving} onMove={() => void move(target)} onCancel={() => setTarget(null)} />}
    </Group>
  );
}

function CountRow({ label, value }: { label: string; value: string }) {
  return (
    <Row label={label}>
      <span className="mono selectable">{value}</span>
    </Row>
  );
}

/** `claude mcp add` with the binary quoted when its path has spaces (the app bundle's does). */
function mcpAddCommand(binaryPath: string): string {
  const binary = /\s/.test(binaryPath) ? `"${binaryPath}"` : binaryPath;
  return `claude mcp add timetracker -- ${binary} mcp`;
}

function DataPage({ s, set }: SettingsPageProps) {
  const dataVersion = useStore((s) => s.dataVersion);
  const { data: info, reload } = useApiResult(`data-info:${dataVersion}`, api.dataInfo);
  if (!info) return null;
  return (
    <>
      {!info.available && (
        <div className="settings-alert selectable" role="alert">
          The data store could not be opened, so nothing is recorded: {info.error}
        </div>
      )}
      <FolderGroup info={info} onMoved={reload} />
      {info.available && (
        <Group title="Contents">
          <CountRow label="Activity spans" value={formatCount(info.activities)} />
          <CountRow label="Recordings" value={formatCount(info.recordings)} />
          <CountRow label="Projects" value={formatCount(info.projects)} />
          <CountRow label="First activity" value={info.firstActivity === null ? "None yet" : formatDay(info.firstActivity)} />
          <CountRow label="Size on disk" value={formatBytes(info.sizeBytes)} />
        </Group>
      )}
      <Group title="Backup">
        <Row label="JSON backup" description="Every client, project, tag, activity span and recording in one file.">
          <button type="button" className="btn" disabled={!info.available} onClick={() => void exportBackup()}>
            <Download size="1.077rem" /> Export backup…
          </button>
        </Row>
      </Group>
      <Group title="MCP server">
        <Switch
          label="Expose activity to the MCP server"
          description="Window titles and URLs become readable by MCP clients such as Claude. Recordings and reports are always available."
          checked={s.mcp.exposeActivity}
          onChange={(exposeActivity) => set({ mcp: { ...s.mcp, exposeActivity } })}
        />
        <Row label="Claude" description={<span className="mono selectable">{mcpAddCommand(info.binaryPath)}</span>}>
          <button type="button" className="btn" onClick={() => void copyText(mcpAddCommand(info.binaryPath), "the command")}>
            <Copy size="1.077rem" /> Copy
          </button>
        </Row>
      </Group>
    </>
  );
}

export function registerCoreSettingsPages() {
  registerSettingsPage({ id: "appearance", group: "Interface", label: "Appearance", icon: Palette, order: 10, intro: "How TimeTracker looks. Changes apply right away.", Body: AppearancePage });
  registerSettingsPage({ id: "shortcuts", group: "Interface", label: "Shortcuts", icon: Keyboard, order: 20, intro: "Click a shortcut and press the new keys. Backspace unbinds, Escape cancels.", Body: ShortcutsPage });
  registerSettingsPage({
    id: "capture",
    group: "Tracking",
    label: "Capture",
    icon: EyeOff,
    order: 10,
    intro: "What the tracker records. It never records keystrokes, screenshots, audio, your clipboard or page content, and nothing leaves this Mac.",
    Body: CapturePage,
  });
  registerSettingsPage({ id: "tracker", group: "Tracking", label: "Tracker", icon: Activity, order: 20, intro: "When time counts as away, and how often the tracker looks.", Body: TrackerPage });
  registerSettingsPage({ id: "timeline", group: "Tracking", label: "Timeline", icon: GanttChart, order: 30, intro: "How the Day view shows your activity.", Body: TimelinePage });
  registerSettingsPage({ id: "general", group: "System", label: "General", icon: Power, order: 0, intro: "How TimeTracker starts.", Body: GeneralPage });
  registerSettingsPage({ id: "data", group: "System", label: "Data", icon: Database, order: 15, intro: "Where TimeTracker keeps your activity and recordings. Everything stays on this Mac.", Body: DataPage });
  registerSettingsPage({ id: "logging", group: "System", label: "Logging", icon: ScrollText, order: 20, intro: "What TimeTracker writes to its log file.", Body: LoggingPage });
}
