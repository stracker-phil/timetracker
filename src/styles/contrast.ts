/**
 * WCAG contrast of the palettes' color pairs, mirroring the CSS: `shadeColor` (Accent.tsx) for a project's
 * shade and the `--block-*` formulas in theme.css for a filled recording. For the scheme audit and its tests;
 * the app itself lets CSS compute the colors.
 */
import { shadeLightness } from "../lib/accent";

export type Rgb = [number, number, number];

/** The `--block-*` tokens of a content palette. */
export interface BlockTokens {
  fillL: number;
  fillSpread: number;
  fillC: number;
  textL: number;
  textC: number;
}

export function hexToRgb(hex: string): Rgb {
  const h = hex.replace("#", "");
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255) as Rgb;
}

const toLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const fromLinear = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);
const clamp01 = (c: number) => Math.min(1, Math.max(0, c));

/** OKLCH of an sRGB color: lightness 0–1, chroma, hue in degrees. */
export function rgbToOklch([r, g, b]: Rgb): [number, number, number] {
  const [lr, lg, lb] = [r, g, b].map(toLinear);
  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb);
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb);
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb);
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return [L, Math.hypot(A, B), ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360];
}

/** sRGB of an OKLCH color, clipped into the gamut. */
export function oklchToRgb(L: number, C: number, H: number): Rgb {
  const a = C * Math.cos((H * Math.PI) / 180);
  const b = C * Math.sin((H * Math.PI) / 180);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const linear = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
  return linear.map((c) => fromLinear(clamp01(c))) as Rgb;
}

function luminance(rgb: Rgb): number {
  const [r, g, b] = rgb.map(toLinear);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** The WCAG contrast ratio, 1 to 21. */
export function contrastRatio(a: Rgb, b: Rgb): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/** `color` at `alpha` over `bg`, as `color-mix(in srgb, color X%, transparent)` composites. */
export function over(color: Rgb, alpha: number, bg: Rgb): Rgb {
  return color.map((c, i) => c * alpha + bg[i] * (1 - alpha)) as Rgb;
}

/** A filled recording of `accent` (hex) at `shade`: fill, text and meta, as theme.css derives them. */
export function blockColors(accent: string, shade: number, t: BlockTokens): { fill: Rgb; text: Rgb; meta: Rgb } {
  const [, chroma, hue] = rgbToOklch(hexToRgb(accent));
  const fill = oklchToRgb(t.fillL + (shadeLightness(shade) - 0.6) * t.fillSpread, Math.min(chroma, t.fillC), hue);
  const text = oklchToRgb(t.textL, Math.min(chroma, t.textC), hue);
  return { fill, text, meta: over(text, 0.75, fill) };
}
