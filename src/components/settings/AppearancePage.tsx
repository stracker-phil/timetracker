/** Settings → Appearance: the scheme per macOS mode, the accent, date titles and the text size. */
import { TEXT_SIZES, type Scheme } from "../../api";
import { schemeLook } from "../../lib/theme";
import type { SettingsPageProps, SettingsRow } from "../../registry/settingsPages";
import { AccentSwatches } from "../Accent";
import { Group, Row, Segmented } from "./controls";

const SCHEME_LABELS: Record<Scheme, string> = { default: "Default", light: "Light", dark: "Dark" };

/** One tile per scheme: a miniature window in the scheme's look. */
function SchemePicker({ label, value, onChange }: { label: string; value: Scheme; onChange: (scheme: Scheme) => void }) {
  return (
    <div className="scheme-picker" role="group" aria-label={label}>
      {(Object.keys(SCHEME_LABELS) as Scheme[]).map((id) => {
        const look = schemeLook(id);
        return (
          <button key={id} type="button" className="scheme-option" aria-pressed={value === id} onClick={() => onChange(id)}>
            <span className="scheme-tile" data-content={look.content} data-sidebar={look.sidebar} aria-hidden="true">
              <span className="scheme-tile-sidebar">
                <span className="scheme-tile-entry" />
              </span>
              <span className="scheme-tile-content">
                <span className="scheme-tile-block" />
              </span>
            </span>
            {SCHEME_LABELS[id]}
          </button>
        );
      })}
    </div>
  );
}

export const APPEARANCE_ROWS: SettingsRow[] = [
  { id: "light-scheme", title: "Light mode scheme", keywords: ["theme", "look", "colors"] },
  { id: "dark-scheme", title: "Dark mode scheme", keywords: ["theme", "look", "colors", "dark mode"] },
  { id: "accent", title: "Accent", keywords: ["color", "colour", "tint", "highlight"] },
  { id: "calendar-title", title: "Calendar title", keywords: ["month", "date format"] },
  { id: "day-title", title: "Day title", keywords: ["weekday", "date format"] },
  { id: "text-size", title: "Text size", keywords: ["font", "zoom", "larger", "smaller"] },
];

export function AppearancePage({ s, set }: SettingsPageProps) {
  return (
    <>
      <Group title="Look">
        <Row label="Light mode scheme" anchor="light-scheme" description="Used while macOS is in light mode." stacked>
          <SchemePicker label="Light mode scheme" value={s.lightScheme} onChange={(lightScheme) => set({ lightScheme })} />
        </Row>
        <Row label="Dark mode scheme" anchor="dark-scheme" description="Used while macOS is in dark mode." stacked>
          <SchemePicker label="Dark mode scheme" value={s.darkScheme} onChange={(darkScheme) => set({ darkScheme })} />
        </Row>
        <Row label="Accent" anchor="accent" description="Buttons, links, selection and focus. System follows macOS.">
          <AccentSwatches withSystem value={s.accent} onChange={(accent) => set({ accent })} />
        </Row>
      </Group>
      <Group title="Dates">
        <Segmented
          label="Calendar title"
          anchor="calendar-title"
          description="The sidebar calendar's month: abbreviated or in full."
          options={["short", "full"] as const}
          value={s.calendarTitle}
          onChange={(calendarTitle) => set({ calendarTitle })}
        />
        <Segmented
          label="Day title"
          anchor="day-title"
          description="The Day view's title: with the weekday and month in full, or abbreviated."
          options={["long", "short"] as const}
          value={s.dayTitle}
          onChange={(dayTitle) => set({ dayTitle })}
        />
      </Group>
      <Group title="Text">
        <Segmented
          label="Text size"
          anchor="text-size"
          description="Every text in the window. ⌘= and ⌘- step through the sizes outside the Day view."
          options={TEXT_SIZES.map((t) => t.size)}
          value={s.textSize}
          onChange={(textSize) => set({ textSize })}
        />
      </Group>
    </>
  );
}
