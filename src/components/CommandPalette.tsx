import { useRef, useState } from "react";
import { Search } from "lucide-react";
import { useStore } from "../state/store";
import { Modal } from "./Modal";
import { PaletteFooter, PaletteList } from "./palette/PaletteList";
import { usePaletteItems } from "./palette/usePaletteItems";

/** ⌘K: views, dates, projects, clients, saved reports, settings pages and commands (docs/frontend.md). */
export function CommandPalette() {
  const open = useStore((s) => s.paletteOpen);
  const seed = useStore((s) => s.paletteSeed);
  // Mounted per opening (and per seed, so a command run from the palette can reopen it with a prefix).
  return open ? <PaletteSheet key={seed} /> : null;
}

function PaletteSheet() {
  const seed = useStore((s) => s.paletteSeed);
  const close = () => useStore.getState().setPaletteOpen(false);
  const [query, setQuery] = useState(seed);
  const [index, setIndex] = useState(0);
  const activeRef = useRef<HTMLLIElement>(null);
  const { items } = usePaletteItems(query);
  // Results can shrink under the selection (the dates arrive from Rust, a narrower query).
  const active = Math.max(0, Math.min(index, items.length - 1));

  const choose = (i: number, alt = false) => {
    const item = items[i];
    const run = alt ? (item?.goAlt ?? item?.go) : item?.go;
    run?.();
  };

  return (
    <Modal onClose={close} variant="panel" className="palette" label="Command palette">
      <div className="palette-input">
        <Search size="1.077rem" aria-hidden="true" />
        <input
          autoFocus
          aria-label="Search"
          placeholder="View, date, project, client, report or command…"
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
              choose(active, e.metaKey);
            }
          }}
        />
      </div>
      <PaletteList items={items} active={active} activeRef={activeRef} onHover={setIndex} onChoose={choose} />
      <PaletteFooter item={items[active]} empty={!query.trim()} />
    </Modal>
  );
}
