import { ACCENTS, SHADES } from "../api";
import { shadeLightness } from "../lib/accent";

/** One swatch per accent hue; `value` is pressed (`null`: none is). */
export function AccentSwatches({ value, onChange }: { value: string | null; onChange: (accent: string) => void }) {
  return (
    <div className="accent-swatches" role="group" aria-label="Accent">
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

/** A small filled circle in an accent hue, or in one of its shades, e.g. a project's color. */
export function AccentDot({ accent, shade }: { accent: string; shade?: number }) {
  const background = shade === undefined ? `var(--a-${accent})` : shadeColor(accent, shade);
  return <span className="accent-dot" aria-hidden="true" style={{ background }} />;
}
