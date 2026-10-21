import { useEffect, useRef, useState } from "react";
import { CornerDownLeft, Search } from "lucide-react";
import { activeScopes, allCommands, bindingFor, bindingLabel, runCommand } from "../commands";
import type { Keybindings, ProjectListItem } from "../api";
import { projectLabel, searchProjects } from "../state/slices/projects";
import { useStore } from "../state/store";
import { AccentDot } from "./Accent";
import { KeyCap } from "./Field";
import { Modal } from "./Modal";

interface Item {
  id: string;
  title: string;
  keys: string;
  /** A project's color dot. */
  accent?: string;
  shade?: number;
  run: () => void;
}

const PROJECT_PREFIX = "p:";

/** ⌘K: commands of the active scopes, then active projects once something is typed; `>` and `p:` limit to one. */
export function CommandPalette() {
  const open = useStore((s) => s.paletteOpen);
  const seed = useStore((s) => s.paletteSeed);
  // Mounted per opening (and per seed, so a command run from the palette can reopen it with a prefix).
  return open ? <PaletteSheet key={seed} /> : null;
}

function commandItems(query: string, custom: Keybindings): Item[] {
  const needle = query.toLowerCase();
  const scopes = activeScopes();
  return allCommands()
    .filter((c) => c.id !== "palette.toggle" && scopes.includes(c.scope) && (c.when?.() ?? true))
    .filter((c) => !needle || c.title.toLowerCase().includes(needle) || c.id.toLowerCase().includes(needle))
    .map((c) => ({ id: c.id, title: c.title, keys: bindingLabel(bindingFor(c.id, custom)), run: () => void runCommand(c.id) }));
}

/** `Project: Client / Name` entries; Enter opens the project's sheet. */
function projectItems(query: string, projects: ProjectListItem[]): Item[] {
  return searchProjects(projects, query).map((p) => ({
    id: `project:${p.project.id}`,
    title: `Project: ${projectLabel(p)}`,
    keys: "",
    accent: p.project.color,
    shade: p.project.shade,
    run: () => useStore.getState().openDialog({ type: "project", id: p.project.id }),
  }));
}

function paletteItems(query: string, custom: Keybindings, projects: ProjectListItem[]): Item[] {
  const q = query.trimStart();
  if (q.startsWith(">")) return commandItems(q.slice(1).trim(), custom);
  if (q.startsWith(PROJECT_PREFIX)) return projectItems(q.slice(PROJECT_PREFIX.length).trim(), projects);
  // Projects only once something is typed, so the empty palette stays a command list.
  const needle = q.trim();
  return [...commandItems(needle, custom), ...(needle ? projectItems(needle, projects) : [])];
}

function PaletteSheet() {
  const seed = useStore((s) => s.paletteSeed);
  const custom = useStore((s) => s.keybindings);
  const projects = useStore((s) => s.projects);
  const close = () => useStore.getState().setPaletteOpen(false);
  const [query, setQuery] = useState(seed);
  const [index, setIndex] = useState(0);
  const activeRef = useRef<HTMLLIElement>(null);
  const items = paletteItems(query, custom, projects);
  const active = Math.max(0, Math.min(index, items.length - 1));

  useEffect(() => {
    activeRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [active]);

  const choose = (i: number) => {
    const item = items[i];
    if (!item) return;
    close();
    item.run();
  };

  return (
    <Modal onClose={close} variant="panel" className="palette" label="Command palette">
      <div className="palette-input">
        <Search size="1.077rem" aria-hidden="true" />
        <input
          autoFocus
          aria-label="Search commands"
          placeholder="Type a command or a project…"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setIndex(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setIndex(Math.min(items.length - 1, active + 1));
            }
            if (e.key === "ArrowUp") {
              e.preventDefault();
              setIndex(Math.max(0, active - 1));
            }
            if (e.key === "Enter") {
              e.preventDefault();
              choose(active);
            }
          }}
        />
      </div>
      <ul className="palette-list" role="listbox" aria-label="Results">
        {items.map((it, i) => (
          <li
            key={it.id}
            ref={i === active ? activeRef : undefined}
            role="option"
            aria-selected={i === active}
            className={i === active ? "active" : ""}
            onMouseEnter={() => setIndex(i)}
            onClick={() => choose(i)}
          >
            {it.accent && <AccentDot accent={it.accent} shade={it.shade} />}
            <span className="grow-text">{it.title}</span>
            <KeyCap keys={it.keys} />
            {i === active && <CornerDownLeft size="0.923rem" className="muted" />}
          </li>
        ))}
        {items.length === 0 && <li className="muted">No matches.</li>}
      </ul>
      <div className="palette-foot o3">
        <span>↵ run · esc close</span>
        <span className="mono">&gt; commands · p: projects</span>
      </div>
    </Modal>
  );
}
