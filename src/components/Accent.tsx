import { ACCENTS } from "../api";

/** One swatch per accent hue; `value` is pressed (`null`: none is). */
export function AccentSwatches({ value, small, onChange }: { value: string | null; small?: boolean; onChange: (accent: string) => void }) {
  return (
    <div className={small ? "accent-swatches small" : "accent-swatches"} role="group" aria-label="Accent">
      {ACCENTS.map((a) => (
        <button key={a} type="button" aria-label={`Accent ${a}`} aria-pressed={value === a} title={a} style={{ background: `var(--a-${a})` }} onClick={() => onChange(a)} />
      ))}
    </div>
  );
}
