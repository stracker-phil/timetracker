/** Settings → Shortcuts: every command's key, grouped by where the key works (ADR 0050). */
import { useState, type ReactNode } from "react";
import { RotateCcw, Search, X } from "lucide-react";
import { allCommands, bind, bindingFor, bindingFromEvent, bindingLabel, holderOf, isDefaultBinding, reassign, scopeLabel, type Command } from "../../commands";
import type { SettingsRow } from "../../registry/settingsPages";
import { useStore } from "../../state/store";
import { InsetField } from "../Field";
import { Modal } from "../Modal";
import { Group, Row } from "./controls";
import { useKeybindings, useSettings } from "../../queries/settings";

/** Groups in this order; any other scope follows them. */
const SCOPE_ORDER = ["global", "dated", "day", "suggestion", "week", "month", "reports", "projects", "tags", "rules", "settings"];

const SPECIAL_TITLES: Record<string, string> = {
  global: "In the main window",
  dated: "In Day, Week, Month and Reports",
  suggestion: "With a suggestion focused",
  settings: "In the Settings window",
};

const groupTitle = (scope: string) => SPECIAL_TITLES[scope] ?? `In the ${scopeLabel(scope)} view`;

const scopeRank = (scope: string) => {
  const index = SCOPE_ORDER.indexOf(scope);
  return index === -1 ? SCOPE_ORDER.length : index;
};

const GLOBAL_TITLE = "Open timer picker";

const shortcutAnchor = (id: string) => `shortcut:${id}`;

const listedCommands = () => allCommands().filter((c) => !c.menuOnly);

/** Every row the Settings search finds on this page: the system-wide key and one row per command. */
export function shortcutRows(): SettingsRow[] {
  return [
    { id: "global-shortcut", title: GLOBAL_TITLE, keywords: ["system-wide", "global", "hotkey", "Start timer"] },
    ...listedCommands().map((c) => ({ id: shortcutAnchor(c.id), title: c.title })),
  ];
}

function ShortcutField({ value, onChange }: { value: string | null; onChange: (binding: string | null) => void }) {
  const [recording, setRecording] = useState(false);
  return (
    <button
      type="button"
      data-recording={recording || undefined}
      className={`shortcut-field${recording ? " recording" : ""}${value === null && !recording ? " unbound" : ""}`}
      aria-label={recording ? "Press the new shortcut" : "Record shortcut"}
      onClick={(e) => {
        e.currentTarget.focus();
        setRecording(true);
      }}
      onBlur={() => setRecording(false)}
      onKeyDown={(e) => {
        if (!recording) return;
        e.preventDefault();
        e.stopPropagation();
        if (e.key === "Escape") return setRecording(false);
        if (e.key === "Backspace" || e.key === "Delete") {
          onChange(null);
          return setRecording(false);
        }
        const binding = bindingFromEvent(e);
        if (!binding) return;
        onChange(binding);
        setRecording(false);
      }}
    >
      {recording ? "Press keys…" : value ? bindingLabel(value) : "unbound"}
    </button>
  );
}

/** A small icon button that keeps its slot, invisible, while it doesn't apply, so the row never shifts. */
function SlotButton({ shown, label, title, onClick, children }: { shown: boolean; label: string; title: string; onClick: () => void; children: ReactNode }) {
  if (!shown) return <span className="icon-btn tiny slot-empty" aria-hidden="true" />;
  return (
    <button type="button" className="icon-btn tiny" aria-label={label} title={title} onClick={onClick}>
      {children}
    </button>
  );
}

/** The system-wide key that opens the timer picker; Rust registers it with the system. */
function GlobalShortcut() {
  const binding = useSettings()?.timerShortcut ?? null;
  const [error, setError] = useState<string | null>(null);
  const change = async (next: string | null) => setError(await useStore.getState().saveTimerShortcut(next));
  return (
    <Group title="System-wide">
      <Row label={GLOBAL_TITLE} anchor="global-shortcut" description={error ? <span role="alert">{error}</span> : "From any app, even with TimeTracker in the background."}>
        <div className="shortcut-control">
          <SlotButton shown={binding !== null} label={`Remove shortcut for ${GLOBAL_TITLE}`} title="Remove shortcut" onClick={() => void change(null)}>
            <X size="0.846rem" />
          </SlotButton>
          <ShortcutField value={binding} onChange={(b) => void change(b)} />
        </div>
      </Row>
    </Group>
  );
}

interface Pending {
  command: Command;
  binding: string;
  holder: Command;
}

/** Asks before a key moves from the command that holds it; Reassign is one save, Cancel keeps both. */
function ReassignSheet({ pending, onReassign, onCancel }: { pending: Pending; onReassign: () => void; onCancel: () => void }) {
  const key = bindingLabel(pending.binding);
  return (
    <Modal onClose={onCancel} labelledBy="reassign-title">
      <h2 id="reassign-title">
        {key} is used by “{pending.holder.title}”
      </h2>
      <p>
        Reassign it to “{pending.command.title}”? “{pending.holder.title}” will have no shortcut.
      </p>
      <div className="actions">
        <button type="button" className="btn" onClick={onCancel}>
          Cancel
        </button>
        <button type="button" className="btn btn-primary" autoFocus onClick={onReassign}>
          Reassign
        </button>
      </div>
    </Modal>
  );
}

function CommandRow({ command, bindings, onRecord }: { command: Command; bindings: Record<string, string | null>; onRecord: (binding: string | null) => void }) {
  const binding = bindingFor(command.id, bindings);
  return (
    <Row label={command.title} anchor={shortcutAnchor(command.id)}>
      <div className="shortcut-control">
        <SlotButton shown={!isDefaultBinding(command.id, bindings)} label={`Reset ${command.title}`} title="Reset to default" onClick={() => onRecord(bindingFor(command.id, {}))}>
          <RotateCcw size="0.846rem" />
        </SlotButton>
        <SlotButton shown={binding !== null} label={`Remove shortcut for ${command.title}`} title="Remove shortcut" onClick={() => onRecord(null)}>
          <X size="0.846rem" />
        </SlotButton>
        <ShortcutField value={binding} onChange={onRecord} />
      </div>
    </Row>
  );
}

/** The listed commands matching the page's search, grouped by scope in `SCOPE_ORDER`. */
function commandGroups(needle: string): { scope: string; commands: Command[] }[] {
  const matching = listedCommands().filter((c) => !needle || c.title.toLowerCase().includes(needle) || c.id.toLowerCase().includes(needle));
  const scopes = [...new Set(matching.map((c) => c.scope))].sort((a, b) => scopeRank(a) - scopeRank(b) || a.localeCompare(b));
  return scopes.map((scope) => ({ scope, commands: matching.filter((c) => c.scope === scope) }));
}

export function ShortcutsPage() {
  const bindings = useKeybindings();
  const save = useStore((s) => s.saveKeybindings);
  const [search, setSearch] = useState("");
  const [pending, setPending] = useState<Pending | null>(null);
  const groups = commandGroups(search.trim().toLowerCase());

  /** Saves a recorded key (or Reset's default), or asks first when another command holds it. */
  const record = (command: Command, binding: string | null) => {
    const holder = binding ? holderOf(command.id, binding, bindings) : undefined;
    if (binding && holder) setPending({ command, binding, holder });
    else void save(bind(bindings, command.id, binding));
  };
  const confirmReassign = (move: Pending) => {
    void save(reassign(bindings, move.command.id, move.binding, move.holder.id));
    setPending(null);
  };

  return (
    <>
      <InsetField icon={<Search size="1.077rem" />} className="settings-search">
        <input aria-label="Search shortcuts" placeholder="Search commands" value={search} onChange={(e) => setSearch(e.target.value)} />
      </InsetField>
      <GlobalShortcut />
      {groups.length === 0 && <p className="settings-intro">No matching commands.</p>}
      {groups.map(({ scope, commands }) => (
        <Group key={scope} title={groupTitle(scope)}>
          {commands.map((c) => (
            <CommandRow key={c.id} command={c} bindings={bindings} onRecord={(b) => record(c, b)} />
          ))}
        </Group>
      ))}
      {pending && (
        <ReassignSheet
          pending={pending}
          onCancel={() => setPending(null)}
          onReassign={() => confirmReassign(pending)}
        />
      )}
    </>
  );
}
