import { useMemo, useState } from "react";
import type { Settings } from "../../api";
import { resolveSettingsPage, searchSettings, settingsGroups, useSettingsPagesVersion, type SettingsMatch } from "../../registry/settingsPages";
import { useStore } from "../../state/store";
import { ScrollTarget, type RevealTarget } from "./controls";
import { currentSettings, useSettings } from "../../queries/settings";

/** What Settings is asked to show: a page or row id. A new object per request, so asking again reveals again. */
export interface SettingsRequest {
  id?: string;
}

const targetOf = (match: SettingsMatch) => match.row?.id ?? match.page.id;

/** Saves the page's change quietly on top of the settings as they are now; see `SettingsPageProps`. */
async function savePatch(patch: Partial<Settings>, returnError: boolean): Promise<string | null> {
  const settings = currentSettings();
  return settings ? useStore.getState().saveSettings({ ...settings, ...patch }, { quiet: true, returnError }) : null;
}

const set = (patch: Partial<Settings>) => void savePatch(patch, false);
const commit = (patch: Partial<Settings>) => savePatch(patch, true);

/** A page in the list, or a search hit: a row hit shows its page and, below, the row's title. */
function NavButton({ page, row, current, onOpen }: SettingsMatch & { current: string; onOpen: (id: string) => void }) {
  const { icon: Icon } = page;
  return (
    <button type="button" aria-label={row ? `${row.title}, ${page.label}` : undefined} aria-current={!row && current === page.id ? "page" : undefined} onClick={() => onOpen(targetOf({ page, row }))}>
      <Icon size="1.077rem" aria-hidden="true" />
      <span className="settings-nav-text">
        <span className="settings-nav-title">{page.label}</span>
        {row && <span className="settings-nav-row">{row.title}</span>}
      </span>
    </button>
  );
}

/** The page list under its group headings; typing replaces it with every matching page and row, best first. */
function SettingsNav({ current, onOpen }: { current: string; onOpen: (id: string) => void }) {
  const [query, setQuery] = useState("");
  const searching = query.trim() !== "";
  const results = searching ? searchSettings(query) : [];
  return (
    <nav className="settings-nav" aria-label="Settings pages">
      <div className="settings-titlebar" data-tauri-drag-region />
      <input
        className="input settings-nav-search"
        type="search"
        aria-label="Search Settings"
        placeholder="Search"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && results[0]) onOpen(targetOf(results[0]));
        }}
      />
      <div className="settings-nav-list scroller">
        {searching && results.length === 0 && <p className="settings-nav-empty">No Results</p>}
        {results.length > 0 && (
          <div className="settings-nav-group" role="group" aria-label="Results">
            {results.map((match) => (
              <NavButton key={targetOf(match)} {...match} current={current} onOpen={onOpen} />
            ))}
          </div>
        )}
        {!searching &&
          settingsGroups().map(({ group, pages }) => (
            <div key={group} className="settings-nav-group" role="group" aria-label={group}>
              <div className="settings-nav-heading" aria-hidden="true">
                {group}
              </div>
              {pages.map((page) => (
                <NavButton key={page.id} page={page} current={current} onOpen={onOpen} />
              ))}
            </div>
          ))}
      </div>
    </nav>
  );
}

/** The Settings window: the page list beside the page. `request.id` may name a row: its page opens scrolled to it,
 *  washed in the accent once. */
export function SettingsWindow({ request, onOpen }: { request: SettingsRequest; onOpen: (id: string) => void }) {
  useSettingsPagesVersion();
  const settings = useSettings();
  const resolved = resolveSettingsPage(request.id);
  // One target per request, so a re-render doesn't reveal the row again but asking again does.
  const target = useMemo((): RevealTarget | undefined => {
    const row = resolveSettingsPage(request.id)?.row;
    return row && { id: row.id, under: row.under };
  }, [request]);
  if (!settings || !resolved) return null;
  const { page } = resolved;
  const { Body } = page;

  return (
    <div className="settings-window">
      <SettingsNav current={page.id} onOpen={onOpen} />
      <div className="settings-pane">
        <header data-tauri-drag-region>
          <h2 data-tauri-drag-region>{page.label}</h2>
        </header>
        <div className="settings-body scroller" key={page.id}>
          {page.intro && <p className="settings-intro">{page.intro}</p>}
          <ScrollTarget.Provider value={target}>
            <Body s={settings} set={set} commit={commit} />
          </ScrollTarget.Provider>
        </div>
      </div>
    </div>
  );
}
