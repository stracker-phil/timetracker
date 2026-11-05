import { useState } from "react";
import type { Settings } from "../../api";
import { resolveSettingsPage, settingsGroups, useSettingsPagesVersion, type SettingsNavEntry } from "../../registry/settingsPages";
import { useStore } from "../../state/store";
import { SectionTarget } from "./controls";
import { currentSettings, useSettings } from "../../queries/settings";

const targetOf = (match: SettingsNavEntry) => match.section?.id ?? match.page.id;

/** Saves the page's change quietly on top of the settings as they are now; see `SettingsPageProps`. */
async function savePatch(patch: Partial<Settings>, returnError: boolean): Promise<string | null> {
  const settings = currentSettings();
  return settings ? useStore.getState().saveSettings({ ...settings, ...patch }, { quiet: true, returnError }) : null;
}

const set = (patch: Partial<Settings>) => void savePatch(patch, false);
const commit = (patch: Partial<Settings>) => savePatch(patch, true);

/** The page list, narrowed by the search field; a page found by a section opens at that section. */
function SettingsNav({ current, onPage }: { current: string; onPage: (id: string) => void }) {
  const [query, setQuery] = useState("");
  const groups = settingsGroups(query);
  const first = groups[0]?.pages[0];
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
          if (e.key === "Enter" && first) onPage(targetOf(first));
        }}
      />
      <div className="settings-nav-list scroller">
        {groups.length === 0 && <p className="settings-nav-empty">No Results</p>}
        {groups.map((g) => (
          <div key={g.group} className="settings-nav-group">
            <div className="settings-nav-heading">{g.group}</div>
            {g.pages.map((match) => {
              const { page: { id, label, icon: Icon }, section } = match;
              return (
                <button
                  key={id}
                  type="button"
                  aria-label={section ? `${label} ${section.title}` : undefined}
                  aria-current={current === id ? "page" : undefined}
                  onClick={() => onPage(targetOf(match))}
                >
                  <Icon size="1.077rem" /> {label}
                  {section && <span className="settings-nav-section">{section.title}</span>}
                </button>
              );
            })}
          </div>
        ))}
      </div>
    </nav>
  );
}

/** The Settings window: the page list beside the page. `page` may name a section; the page then opens
 *  scrolled to that section's group. Every change applies right away. */
export function SettingsWindow({ page: shown, onPage }: { page: string | undefined; onPage: (id: string) => void }) {
  useSettingsPagesVersion();
  const settings = useSettings();
  const resolved = resolveSettingsPage(shown);
  if (!settings || !resolved) return null;
  const { page, section } = resolved;
  const { Body } = page;

  return (
    <div className="settings-window">
      <SettingsNav current={page.id} onPage={onPage} />
      <div className="settings-pane">
        <header data-tauri-drag-region>
          <h2 data-tauri-drag-region>{page.label}</h2>
        </header>
        <div className="settings-body scroller" key={page.id}>
          {page.intro && <p className="settings-intro">{page.intro}</p>}
          <SectionTarget.Provider value={section}>
            <Body s={settings} set={set} commit={commit} />
          </SectionTarget.Provider>
        </div>
      </div>
    </div>
  );
}
