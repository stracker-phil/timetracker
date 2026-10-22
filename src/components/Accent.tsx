import { ACCENTS, SHADES, SYSTEM_ACCENT } from "../api";
import { shadeLightness } from "../lib/accent";

/** One swatch per accent hue, after a System one (the macOS accent) with `withSystem`; `value` is pressed (`null`: none is). */
export function AccentSwatches({ value, onChange, withSystem = false }: { value: string | null; onChange: (accent: string) => void; withSystem?: boolean }) {
  return (
    <div className="accent-swatches" role="group" aria-label="Accent">
      {withSystem && (
        <button type="button" className="accent-system" aria-label="Accent system" aria-pressed={value === SYSTEM_ACCENT} title="System" onClick={() => onChange(SYSTEM_ACCENT)} />
      )}
      {ACCENTS.map((a) => (
        <button key={a} type="button" aria-label={`Accent ${a}`} aria-pressed={value === a} title={a} style={{ background: `var(--a-${a})` }} onClick={() => onChange(a)} />
      ))}
    </div>
  );
}

/** The accent's chroma and hue at the lightness of `shade`. */
export function shadeColor(accent: string, shade: number): string {
  return `oklch(from var(--a-${accent}) ${shadeLightness(shade).toFixed(2)} c h)`;
}

/** One swatch per shade of `accent`; `value` is pressed (`null`: none is, the shade is picked on save). */
export function ShadeSwatches({ accent, value, onChange }: { accent: string; value: number | null; onChange: (shade: number) => void }) {
  return (
    <div className="accent-swatches" role="group" aria-label="Shade">
      {Array.from({ length: SHADES }, (_, shade) => (
        <button key={shade} type="button" aria-label={`Shade ${shade + 1}`} aria-pressed={value === shade} style={{ background: shadeColor(accent, shade) }} onClick={() => onChange(shade)} />
      ))}
    </div>
  );
}

export const accentColor = (accent: string, shade?: number | null) => (shade == null ? `var(--a-${accent})` : shadeColor(accent, shade));

/** A small filled circle in an accent hue, or in one of its shades, e.g. a project's color. */
export function AccentDot({ accent, shade }: { accent: string; shade?: number | null }) {
  return <span className="accent-dot" aria-hidden="true" style={{ background: accentColor(accent, shade) }} />;
}
