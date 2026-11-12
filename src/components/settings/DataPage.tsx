/** Settings → Data: where the data lives (and moving it), what it holds, the backup and the MCP server. */
import { Copy, Download, FolderInput, FolderOpen } from "lucide-react";
import { useState } from "react";
import { api, toAppError, type DataInfo } from "../../api";
import { copyText } from "../../lib/clipboard";
import { formatBytes, formatCount } from "../../lib/format";
import type { SettingsPageProps, SettingsRow } from "../../registry/settingsPages";
import { fail } from "../../state/requests";
import { useStore } from "../../state/store";
import { Modal } from "../Modal";
import { exportBackup, revealFolder } from "./actions";
import { Group, Row, Switch } from "./controls";
import { invalidate } from "../../queries/queryClient";
import { SETTINGS_KEY } from "../../queries/settings";
import { DATA_INFO_KEY, useDataInfo } from "../../queries/system";

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
    useStore.getState().toast("success", `Data moved to ${moved.dir}. The old files are still in ${from}.`);
    // The cached settings still name the old folder.
    invalidate([SETTINGS_KEY]);
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
      <Row label="Location" anchor="data-location" stacked>
        <div className="settings-location">
          <div className="settings-buttons">
            <button type="button" className="btn" onClick={() => void revealFolder(info.file)}>
              <FolderOpen size="1.077rem" /> Reveal in Finder
            </button>
            <button type="button" className="btn" disabled={!info.available} onClick={() => void choose()}>
              <FolderInput size="1.077rem" /> Move…
            </button>
          </div>
          <span className="settings-path mono selectable">{info.dir}</span>
        </div>
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
      <span className="tabular selectable">{value}</span>
    </Row>
  );
}

/** `claude mcp add` with the binary quoted when its path has spaces (the app bundle's does). */
function mcpAddCommand(binaryPath: string): string {
  const binary = /\s/.test(binaryPath) ? `"${binaryPath}"` : binaryPath;
  return `claude mcp add timetracker -- ${binary} mcp`;
}

export const DATA_ROWS: SettingsRow[] = [
  { id: "data-location", title: "Location", keywords: ["data folder", "database", "move", "Reveal in Finder"] },
  { id: "contents", title: "Contents", keywords: ["activities", "recordings", "projects", "size on disk", "first activity"] },
  { id: "json-backup", title: "JSON backup", keywords: ["export", "backup"] },
  { id: "expose-activity", title: "Expose activity to the MCP server", keywords: ["MCP", "AI", "window titles"] },
  { id: "mcp-claude", title: "Claude", keywords: ["MCP", "claude mcp add", "command"] },
];

export function DataPage({ s, set }: SettingsPageProps) {
  const { data: info } = useDataInfo();
  if (!info) return null;
  return (
    <>
      {!info.available && (
        <div className="settings-alert selectable" role="alert">
          The data store could not be opened, so nothing is recorded: {info.error}
        </div>
      )}
      <FolderGroup info={info} onMoved={() => invalidate([DATA_INFO_KEY])} />
      {info.available && (
        <Group title="Contents" anchor="contents">
          <CountRow label="Activities" value={formatCount(info.activities)} />
          <CountRow label="Recordings" value={formatCount(info.recordings)} />
          <CountRow label="Projects" value={formatCount(info.projects)} />
          <CountRow label="First activity" value={info.firstActivityLabel ?? "None yet"} />
          <CountRow label="Size on disk" value={formatBytes(info.sizeBytes)} />
        </Group>
      )}
      <Group title="Backup">
        <Row label="JSON backup" anchor="json-backup" description="Every client, project, tag, activity and recording in one file.">
          <button type="button" className="btn" disabled={!info.available} onClick={() => void exportBackup()}>
            <Download size="1.077rem" /> Export backup…
          </button>
        </Row>
      </Group>
      <Group title="MCP server">
        <Switch
          label="Expose activity to the MCP server"
          anchor="expose-activity"
          description="Window titles and URLs become readable by MCP clients such as Claude. Recordings and reports are always available."
          checked={s.mcp.exposeActivity}
          onChange={(exposeActivity) => set({ mcp: { ...s.mcp, exposeActivity } })}
        />
        <Row label="Claude" anchor="mcp-claude" description={<span className="mono selectable">{mcpAddCommand(info.binaryPath)}</span>}>
          <button type="button" className="btn" onClick={() => void copyText(mcpAddCommand(info.binaryPath), "the command")}>
            <Copy size="1.077rem" /> Copy
          </button>
        </Row>
      </Group>
    </>
  );
}
