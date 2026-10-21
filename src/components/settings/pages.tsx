/** The core settings pages. Modules register their own pages from their install function. */
import { Activity, Database, Download, EyeOff, FolderOpen, GanttChart, Keyboard, Palette, Power, ScrollText } from "lucide-react";
import { useId } from "react";
import { api, BUNDLED_FONTS, LOG_LEVELS, SCHEMES, TEXT_SIZES, type LogLevel, type SchemeId } from "../../api";
import { formatBytes, formatCount, formatDay } from "../../lib/format";
import { useApiResult } from "../../lib/useApiResult";
import { registerSettingsPage, type SettingsPageProps } from "../../registry/settingsPages";
import { fail } from "../../state/requests";
import { useStore } from "../../state/store";
import { AccentSwatches } from "../Accent";
import { capitalize, Group, NumberField, Row, Segmented, Switch, TextField } from "./controls";
import { CapturePage, TrackerPage } from "./CapturePage";
import { ShortcutsPage } from "./ShortcutsPage";
import { TimelinePage } from "./TimelinePage";

/** A stored scheme id narrowed to a known one; unknown ids (hand-edited settings) show the first scheme. */
const toSchemeId = (id: string): SchemeId => SCHEMES.find((scheme) => scheme.id === id)?.id ?? SCHEMES[0].id;

/** Two swatches per scheme: its content background and the chosen accent in that scheme. */
function SchemePreview({ scheme, accent }: { scheme: SchemeId; accent: string }) {
  return (
    <span className="scheme-preview" data-scheme={scheme} aria-hidden="true">
      <span style={{ background: "var(--bg)", color: "var(--text)" }}>Aa</span>
      <span style={{ background: `var(--a-${accent})` }} />
    </span>
  );
}

/** The schemes of the slot's own kind come first. */
function SchemeSelect({ label, dark, value, accent, onChange }: { label: string; dark: boolean; value: SchemeId; accent: string; onChange: (id: SchemeId) => void }) {
  const id = useId();
  return (
    <Row label={label} htmlFor={id}>
      <div className="scheme-select">
        <SchemePreview scheme={value} accent={accent} />
        <select id={id} className="input" value={value} onChange={(e) => onChange(toSchemeId(e.target.value))}>
          {[dark, !dark].map((groupDark) => (
            <optgroup key={String(groupDark)} label={groupDark ? "Dark" : "Light"}>
              {SCHEMES.filter((scheme) => scheme.dark === groupDark).map((scheme) => (
                <option key={scheme.id} value={scheme.id}>{scheme.label}</option>
              ))}
            </optgroup>
          ))}
        </select>
      </div>
    </Row>
  );
}

function AppearancePage({ s, set }: SettingsPageProps) {
  return (
    <>
      <Group title="Look">
        <Segmented label="Theme" description="Which scheme applies; System follows macOS." options={["system", "light", "dark"] as const} value={s.theme} onChange={(theme) => set({ theme })} />
        <SchemeSelect label="Light scheme" dark={false} value={toSchemeId(s.lightScheme)} accent={s.accent} onChange={(lightScheme) => set({ lightScheme })} />
        <SchemeSelect label="Dark scheme" dark value={toSchemeId(s.darkScheme)} accent={s.accent} onChange={(darkScheme) => set({ darkScheme })} />
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

function GeneralPage({ s, set }: SettingsPageProps) {
  return (
    <Group>
      <Switch label="Launch at login" description="Starts TimeTracker in the menu bar when you log in, without opening the window." checked={s.launchAtLogin} onChange={(launchAtLogin) => set({ launchAtLogin })} />
    </Group>
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

async function exportBackup() {
  try {
    const path = await api.chooseSavePath(`timetracker-backup-${formatDay(Date.now())}`, "json");
    if (!path) return;
    const written = await api.backupExport(path);
    const count = (n: number) => n.toLocaleString("en-US");
    useStore.getState().toast("success", `Backup written: ${count(written.recordings)} recordings, ${count(written.activities)} activity spans`);
  } catch (e) {
    fail(e, "Backup failed");
  }
}

async function revealFolder(file: string) {
  try {
    await api.revealPath(file);
  } catch (e) {
    fail(e, "Couldn't reveal the folder");
  }
}

function CountRow({ label, value }: { label: string; value: string }) {
  return (
    <Row label={label}>
      <span className="mono selectable">{value}</span>
    </Row>
  );
}

function DataPage() {
  const dataVersion = useStore((s) => s.dataVersion);
  const { data: info } = useApiResult(`data-info:${dataVersion}`, api.dataInfo);
  if (!info) return null;
  return (
    <>
      {!info.available && (
        <div className="settings-alert selectable" role="alert">
          The data store could not be opened, so nothing is recorded: {info.error}
        </div>
      )}
      <Group title="Folder">
        <Row label="Location" description={<span className="mono selectable">{info.dir}</span>}>
          <button type="button" className="btn" onClick={() => void revealFolder(info.file)}>
            <FolderOpen size="1.077rem" /> Reveal in Finder
          </button>
        </Row>
      </Group>
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
  registerSettingsPage({ id: "general", group: "System", label: "General", icon: Power, order: 10, intro: "How TimeTracker starts.", Body: GeneralPage });
  registerSettingsPage({ id: "data", group: "System", label: "Data", icon: Database, order: 15, intro: "Where TimeTracker keeps your activity and recordings. Everything stays on this Mac.", Body: DataPage });
  registerSettingsPage({ id: "logging", group: "System", label: "Logging", icon: ScrollText, order: 20, intro: "What TimeTracker writes to its log file.", Body: LoggingPage });
}
