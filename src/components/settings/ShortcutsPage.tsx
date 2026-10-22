import { useState } from "react";
import { RotateCcw, Search } from "lucide-react";
import { allCommands, bindingFor, bindingFromEvent, bindingLabel, conflicts, scopeLabel } from "../../commands";
import { useStore } from "../../state/store";
import { InsetField } from "../Field";
import { Group, Row } from "./controls";

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

/** The shortcut that opens the timer picker from any app; Rust registers it with the system. */
function GlobalShortcut() {
  const binding = useStore((s) => s.settings?.timerShortcut ?? null);
  const [error, setError] = useState<string | null>(null);
  const change = async (next: string | null) => {
    if (next) setError(await useStore.getState().saveTimerShortcut(next));
  };
  return (
    <Group title="Global shortcut">
      <Row label="Start timer (system-wide)" description={error ? <span role="alert">{error}</span> : "Opens the timer picker from any app."}>
        <ShortcutField value={binding} onChange={(b) => void change(b)} />
      </Row>
    </Group>
  );
}

export function ShortcutsPage() {
  const bindings = useStore((s) => s.keybindings);
  const save = useStore((s) => s.saveKeybindings);
  const [search, setSearch] = useState("");
  const needle = search.trim().toLowerCase();
  const list = allCommands().filter((c) => !c.menuOnly && (!needle || c.title.toLowerCase().includes(needle) || c.id.toLowerCase().includes(needle)));
  const clashes = conflicts(bindings);
  const set = (id: string, binding: string | null) => void save({ ...bindings, [id]: binding });
  const reset = (id: string) => {
    const { [id]: _gone, ...rest } = bindings;
    void save(rest);
  };
  return (
    <>
      <InsetField icon={<Search size="1.077rem" />} className="settings-search">
        <input aria-label="Search shortcuts" placeholder="Search commands" value={search} onChange={(e) => setSearch(e.target.value)} />
      </InsetField>
      {clashes.length > 0 && (
        <div className="shortcut-conflicts" role="alert">
          {clashes.map((c) => (
            <div key={`${c.a.id}:${c.b.id}`}>
              {bindingLabel(c.binding)} is used by both “{c.a.title}” and “{c.b.title}”.
            </div>
          ))}
        </div>
      )}
      <GlobalShortcut />
      <Group title="In TimeTracker">
        {list.length === 0 && <div className="settings-row muted">No matching commands.</div>}
        {list.map((c) => {
          const overridden = Object.hasOwn(bindings, c.id);
          return (
            <Row key={c.id} label={c.title} description={scopeLabel(c.scope)}>
              <div className="shortcut-control">
                {overridden && (
                  <button type="button" className="icon-btn tiny" aria-label={`Reset ${c.title}`} title="Reset to default" onClick={() => reset(c.id)}>
                    <RotateCcw size="0.846rem" />
                  </button>
                )}
                <ShortcutField value={bindingFor(c.id, bindings)} onChange={(b) => set(c.id, b)} />
              </div>
            </Row>
          );
        })}
      </Group>
    </>
  );
}
