// WCAG 2 contrast for OKLCH colours, used by the token tests. No dependencies.
export interface Oklch {
  l: number;
  c: number;
  h: number;
  a: number;
}
export type Rgb = [number, number, number];

export function parseOklch(value: string): Oklch {
  const m = value.trim().match(/^oklch\(\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)\s*(?:\/\s*([\d.]+))?\s*\)$/);
  if (!m) throw new Error(`Not an oklch() colour: ${value}`);
  return { l: +m[1], c: +m[2], h: +m[3], a: m[4] === undefined ? 1 : +m[4] };
}

/** `color-mix(in oklch, a p%, b)`: shorter-hue interpolation, as CSS Color 4 specifies. */
export function mixOklch(a: Oklch, b: Oklch, p: number): Oklch {
  let dh = b.h - a.h;
  if (dh > 180) dh -= 360;
  if (dh < -180) dh += 360;
  const t = 1 - p;
  return { l: a.l + (b.l - a.l) * t, c: a.c + (b.c - a.c) * t, h: (a.h + dh * t + 360) % 360, a: a.a + (b.a - a.a) * t };
}

/** OKLCH → linear sRGB, clamped to the gamut the display will show. */
export function toLinear({ l, c, h }: Oklch): Rgb {
  const hr = (h * Math.PI) / 180;
  const A = c * Math.cos(hr);
  const B = c * Math.sin(hr);
  const l_ = (l + 0.3963377774 * A + 0.2158037573 * B) ** 3;
  const m_ = (l - 0.1055613458 * A - 0.0638541728 * B) ** 3;
  const s_ = (l - 0.0894841775 * A - 1.291485548 * B) ** 3;
  const clamp = (x: number) => Math.min(1, Math.max(0, x));
  return [
    clamp(4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_),
    clamp(-1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_),
    clamp(-0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_),
  ];
}

const encode = (x: number) => (x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055);
const decode = (x: number) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4);

/** Paints `top` (possibly translucent) over an opaque `bottom`, blending in sRGB like the browser. */
export function over(top: Oklch, bottom: Rgb): Rgb {
  const t = toLinear(top).map(encode);
  const b = bottom.map(encode);
  return t.map((x, i) => decode(x * top.a + b[i] * (1 - top.a))) as Rgb;
}

export function contrast(a: Rgb, b: Rgb) {
  const lum = ([r, g, bl]: Rgb) => 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
