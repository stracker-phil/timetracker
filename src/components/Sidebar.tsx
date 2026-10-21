import { useBindingLabel } from "../commands";
import { useStore } from "../state/store";
import { KeyCap } from "./Field";
import { TrackerStatus } from "./TrackerStatus";
import { VIEWS } from "./views";

function NavEntry({ view }: { view: (typeof VIEWS)[number] }) {
  const current = useStore((s) => s.view === view.id);
  const setView = useStore((s) => s.setView);
  const keys = useBindingLabel(`view.${view.id}`);
  const Icon = view.icon;
  return (
    <button type="button" className={current ? "si sel" : "si"} aria-current={current ? "page" : undefined} onClick={() => setView(view.id)}>
      <Icon size="1.077rem" aria-hidden="true" />
      <span className="label">{view.label}</span>
      <KeyCap keys={keys} />
    </button>
  );
}

export function Sidebar() {
  return (
    <aside className="sidebar" aria-label="Sidebar">
      <div className="sidebar-body">
        <nav className="sidebar-nav" aria-label="Views">
          {VIEWS.map((view) => (
            <NavEntry key={view.id} view={view} />
          ))}
        </nav>
      </div>
      <TrackerStatus />
    </aside>
  );
}
