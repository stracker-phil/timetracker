import { type Client, type DateJump, type Keybindings, type PeriodUnit, type Project, type ProjectListItem, type SavedReport } from "../../api";
import { activeScopes, allCommands, bindingFor, bindingLabel } from "../../commands";
import { settingsPages, useSettingsPagesVersion } from "../../registry/settingsPages";
import { projectLabel } from "../../state/slices/projects";
import { useStore, type View } from "../../state/store";
import { VIEWS } from "../views";
import { fuzzyScore, parsePrefix, score, type Prefix } from "./parse";
import { useDateJumps } from "./useDateJumps";

export interface Item {
  key: string;
  /** Go to, Settings, Recent projects, Projects, Clients, Saved reports or Commands. */
  section: string;
  title: string;
  subtitle?: string;
  /** A project's or client's color dot. */
  accent?: string;
  shade?: number;
  /** A command's or view's key cap. */
  keys?: string;
  /** How well it matches the typed text (`score`); sections and their items are ordered by it. */
  rank: number;
  go: () => void;
  /** ⌘↵, named in the footer by `altLabel`. */
  goAlt?: () => void;
  altLabel?: string;
}

const MAX_ITEMS = 80;
const GO_TO = "Go to";

/** Closes the palette before acting, so a command that opens it again (with a seed) wins. */
const closeThen = (run: () => void) => () => {
  useStore.getState().setPaletteOpen(false);
  run();
};

const JUMP_VIEWS: Record<PeriodUnit, View> = { day: "day", week: "week", month: "month" };

function showReport(filter: { clientIds: string[]; projectIds: string[] }) {
  const s = useStore.getState();
  s.setReportFilter({ ...filter, tagIds: [] });
  s.setView("reports");
}

/** The views, each once; today is the Today command (O) and the `Today` date word. */
function viewItems(custom: Keybindings, prefix: Prefix, needle: string): Item[] {
  if (prefix !== null) return [];
  return VIEWS.map(({ id, label }) => ({
    key: `view.${id}`,
    section: GO_TO,
    title: label,
    keys: bindingLabel(bindingFor(`view.${id}`, custom)),
    rank: score([label], needle),
    go: closeThen(() => useStore.getState().setView(id)),
  }));
}

/** Rust matched them already; a typed date is an exact hit. */
function dateItems(jumps: DateJump[], needle: string): Item[] {
  return jumps.map((j) => {
    return {
      key: `date:${j.unit}:${j.from}:${j.name ?? ""}`,
      section: GO_TO,
      title: j.name ?? j.label,
      subtitle: j.name ? j.label : undefined,
      rank: j.name ? Math.max(1, score([j.name], needle)) : 5,
      go: closeThen(() => {
        const s = useStore.getState();
        s.setView(JUMP_VIEWS[j.unit], () => s.showDate(j.from));
      }),
    };
  });
}

function settingsItems(prefix: Prefix, needle: string): Item[] {
  if (prefix !== null || !needle) return [];
  return settingsPages().map((page) => {
    const title = `Settings: ${page.label}`;
    return {
      key: `settings:${page.id}`,
      section: "Settings",
      title,
      rank: score([title, page.label], needle),
      go: closeThen(() => useStore.getState().openDialog({ type: "settings", page: page.id })),
    };
  });
}

/** ↵ the report of the project for the report's period, ⌘↵ a timer on it. */
function projectItem(p: ProjectListItem, section: string, rank: number): Item {
  return {
    key: `project:${p.project.id}`,
    section,
    title: projectLabel(p),
    accent: p.project.color,
    shade: p.project.shade,
    rank,
    go: closeThen(() => showReport({ clientIds: [], projectIds: [p.project.id] })),
    goAlt: closeThen(() => void useStore.getState().startTimer(p.project.id, "", null)),
    altLabel: "start timer",
  };
}

function projectItems(projects: ProjectListItem[], recent: Project[], prefix: Prefix, needle: string): Item[] {
  if (prefix !== null && prefix !== "p:") return [];
  const active = projects.filter((p) => p.project.archivedAt === null);
  if (prefix === null && !needle) {
    return recent.flatMap((r) => active.filter((p) => p.project.id === r.id)).map((p) => projectItem(p, "Recent projects", 1));
  }
  return active.map((p) => projectItem(p, "Projects", fuzzyScore([projectLabel(p), p.project.name], needle)));
}

function clientItems(clients: Client[], prefix: Prefix, needle: string): Item[] {
  if (prefix === "c:" || (prefix === null && needle)) {
    return clients
      .filter((c) => c.archivedAt === null)
      .map((c) => ({
        key: `client:${c.id}`,
        section: "Clients",
        title: c.name,
        accent: c.color,
        rank: fuzzyScore([c.name], needle),
        go: closeThen(() => showReport({ clientIds: [c.id], projectIds: [] })),
      }));
  }
  return [];
}

function savedItems(saved: SavedReport[], prefix: Prefix, needle: string): Item[] {
  if (prefix !== null) return [];
  return saved.map((r) => ({
    key: `saved:${r.id}`,
    section: "Saved reports",
    title: r.name,
    rank: score([r.name], needle),
    go: closeThen(() => useStore.getState().openSavedReport(r)),
  }));
}

/** The active scopes' enabled commands; without `>` the views' commands are left to the views above. */
function commandItems(custom: Keybindings, prefix: Prefix, needle: string): Item[] {
  const listed = prefix === ">" || (prefix === null && needle !== "");
  if (!listed) return [];
  const scopes = activeScopes();
  return allCommands()
    .filter((c) => c.id !== "palette.toggle" && scopes.includes(c.scope) && (c.when?.() ?? true))
    .filter((c) => prefix === ">" || !c.id.startsWith("view."))
    .map((c) => ({
      key: `cmd:${c.id}`,
      section: "Commands",
      title: c.title,
      keys: bindingLabel(bindingFor(c.id, custom)),
      rank: score([c.title], needle),
      go: closeThen(() => void c.run()),
    }));
}

/** Each section's matches best first, the sections by their best match; ties keep the providers' order. */
function arrange(sections: Item[][]): Item[] {
  return sections
    .map((items) => items.filter((i) => i.rank > 0).sort((a, b) => b.rank - a.rank))
    .filter((items) => items.length > 0)
    .sort((a, b) => b[0].rank - a[0].rank)
    .flat()
    .slice(0, MAX_ITEMS);
}

/** The palette's results for `query`, grouped in sections and ranked by how well they match. */
export function usePaletteItems(query: string) {
  const custom = useStore((s) => s.keybindings);
  const projects = useStore((s) => s.projects);
  const recent = useStore((s) => s.recent);
  const clients = useStore((s) => s.clients);
  const saved = useStore((s) => s.savedReports);
  useSettingsPagesVersion();
  const { prefix, needle } = parsePrefix(query);
  const jumps = useDateJumps(prefix, needle);
  const goTo = [...viewItems(custom, prefix, needle), ...dateItems(jumps, needle)];
  const items = arrange([
    goTo,
    settingsItems(prefix, needle),
    projectItems(projects, recent, prefix, needle),
    clientItems(clients, prefix, needle),
    savedItems(saved, prefix, needle),
    commandItems(custom, prefix, needle),
  ]);
  return { items, prefix, needle };
}
