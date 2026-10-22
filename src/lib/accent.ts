/** OKLCH lightness of shade 0 and the step between shades: dark to light, all distinct on either background. */
const DARKEST = 0.36;
const STEP = 0.06;

/** The OKLCH lightness of a project color's `shade`. */
export const shadeLightness = (shade: number) => DARKEST + shade * STEP;
