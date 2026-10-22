import { Search } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { api, type ListRow as Row, type Project } from "../../../api";
import { isTextInput } from "../../../commands";
import { formatHm } from "../../../lib/format";
import { useApiResult } from "../../../lib/useApiResult";
import { selectDayVersion, useStore } from "../../../state/store";
import { useContextMenu } from "../../ContextMenu";
import { InsetField } from "../../Field";
import { pointerDrag } from "../pointerDrag";
import { ListRow } from "./ListRow";
import { MORE_TARGET, dropTargetAt, overflowItems, selectStripProjects } from "./dropTargets";
import { ProjectStrip } from "./ProjectStrip";

const matches = (row: Row, query: string) =>
  [row.app, row.label, row.title, row.url, row.path].some((field) => field?.toLowerCase().includes(query));

/** The editor for the row's whole span; a row with gaps says so, since the recording would cover them. */
function recordRow(row: Row, project: Project | null) {
  const warning =
    row.spans.length > 1 ? `This activity has gaps (${row.spans.length} spans, ${formatHm(row.gapMs)} in between). Adjust the times if needed.` : undefined;
  useStore.getState().openEditor({ mode: "create", start: row.firstStart, end: row.lastEnd, projectId: project?.id, warning });
}

function selectOnTimeline(row: Row) {
  const s = useStore.getState();
  s.focusRecording(null);
  s.setSelection({ start: row.firstStart, end: row.lastEnd, blockIds: row.blockIds });
  s.setDayMode("timeline");
}

/** `reload`: changes when today's open activity may have grown. */
export function ListView({ day, reload }: { day: string; reload: number }) {
  const version = useStore(selectDayVersion(day));
  const result = useApiResult(`list/${day}/${version}/${reload}`, () => api.listView(day), { keepData: true });
  const [query, setQuery] = useState("");
  const [focusedKey, setFocusedKey] = useState<string | null>(null);
  const [openKey, setOpenKey] = useState<string | null>(null);
  const [drag, setDrag] = useState<{ key: string; over: string | null } | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const menu = useContextMenu();

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      const s = useStore.getState();
      if (e.key !== "/" || e.metaKey || e.ctrlKey || isTextInput(e.target) || s.editor || s.dialog || s.paletteOpen) return;
      e.preventDefault();
      searchRef.current?.focus();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const view = result.data?.day === day ? result.data : null;
  if (!view) return result.error ? <p className="settings-alert">{result.error.message}</p> : null;

  const needle = query.trim().toLowerCase();
  const rows = needle ? view.rows.filter((row) => matches(row, needle)) : view.rows;
  const focusedRow = rows.find((r) => r.key === focusedKey) ?? null;
  const pick = focusedRow ? (project: Project) => recordRow(focusedRow, project) : null;

  const focusRow = (index: number) => {
    const clamped = Math.max(0, Math.min(rows.length - 1, index));
    const target = rows[clamped];
    if (!target) return;
    setFocusedKey(target.key);
    listRef.current?.querySelectorAll<HTMLElement>(".list-row")[clamped]?.focus();
  };

  const onRowKeyDown = (e: KeyboardEvent<HTMLLIElement>, row: Row, index: number) => {
    if (e.target !== e.currentTarget) return;
    const actions: Record<string, () => void> = {
      ArrowDown: () => focusRow(index + 1),
      ArrowUp: () => (index === 0 ? searchRef.current?.focus() : focusRow(index - 1)),
      Enter: () => recordRow(row, null),
      " ": () => selectOnTimeline(row),
    };
    const action = actions[e.key];
    if (!action || e.metaKey || e.altKey || e.ctrlKey) return;
    e.preventDefault();
    action();
  };

  const dropOnProject = (row: Row, ev: PointerEvent) => {
    const target = dropTargetAt(ev.clientX, ev.clientY);
    const projects = selectStripProjects(useStore.getState());
    if (target === MORE_TARGET) {
      menu.open({ x: ev.clientX, y: ev.clientY }, overflowItems(projects, (p) => recordRow(row, p)));
      return;
    }
    const project = projects.find((p) => p.id === target);
    if (project) recordRow(row, project);
  };

  const onRowPointerDown = (e: ReactPointerEvent<HTMLLIElement>, row: Row) => {
    if (e.button !== 0) return;
    setFocusedKey(row.key);
    const onTitle = e.target instanceof Element && !!e.target.closest("[data-list-title]");
    pointerDrag(e, {
      move: (_, ev) => setDrag({ key: row.key, over: dropTargetAt(ev.clientX, ev.clientY) }),
      end: (_, dragged, ev) => {
        setDrag(null);
        if (dragged) dropOnProject(row, ev);
        else if (onTitle) setOpenKey((open) => (open === row.key ? null : row.key));
      },
    });
  };

  return (
    <div className={drag ? "list-view dragging" : "list-view"}>
      <div className="list-tools">
        <InsetField icon={<Search size="1.077rem" />} keys="/" className="list-search">
          <input
            ref={searchRef}
            type="search"
            aria-label="Search activity"
            placeholder="Search apps, titles, URLs and paths"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape" && query) {
                e.preventDefault();
                setQuery("");
              } else if (e.key === "ArrowDown") {
                e.preventDefault();
                focusRow(rows.findIndex((r) => r.key === focusedKey));
              }
            }}
          />
        </InsetField>
        <ProjectStrip over={drag?.over ?? null} onPick={pick} openMenu={menu.open} />
      </div>
      {rows.length === 0 ? (
        <p className="list-empty o3">{needle ? "No activity matches" : "No activity on this day"}</p>
      ) : (
        <ul className="list-rows" ref={listRef} aria-label="Activity">
          {rows.map((row, i) => (
            <ListRow
              key={row.key}
              row={row}
              // Without a focused row, the first one takes Tab.
              focused={focusedRow ? row === focusedRow : i === 0}
              open={openKey === row.key}
              dragging={drag?.key === row.key}
              onPointerDown={(e) => onRowPointerDown(e, row)}
              onKeyDown={(e) => onRowKeyDown(e, row, i)}
              onFocus={() => setFocusedKey(row.key)}
              onToggleDetails={() => setOpenKey((open) => (open === row.key ? null : row.key))}
            />
          ))}
        </ul>
      )}
      {menu.element}
    </div>
  );
}
