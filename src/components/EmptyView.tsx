import { useStore } from "../state/store";
import { VIEWS } from "./views";

/** Placeholder for a view that has no content yet. */
export function EmptyView() {
  const view = useStore((s) => s.view);
  const label = VIEWS.find((v) => v.id === view)?.label ?? view;
  return (
    <section className="empty-view" aria-label={label}>
      <h1 className="empty-view-title">{label}</h1>
    </section>
  );
}
