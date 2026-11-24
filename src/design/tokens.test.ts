import { describe, expect, it } from 'vitest';
import { contrast, mixOklch, over, parseOklch, toLinear, type Oklch, type Rgb } from './contrast';
import css from './tokens.css?raw';

/** Primitive values of one style × theme block. */
function primitives(style: string, theme: string) {
  const block = css.match(new RegExp(`\\[data-style='${style}'\\]\\[data-theme='${theme}'\\]\\s*\\{([^}]*)\\}`));
  if (!block) throw new Error(`No block for ${style}/${theme}`);
  const p: Record<string, Oklch> = {};
  for (const [, name, value] of block[1].matchAll(/--p-([\w-]+):\s*(oklch\([^)]*\))/g)) p[name] = parseOklch(value);
  return p;
}

/** Reads the percentage of a `color-mix(in oklch, var(--p-x) N%, var(--p-y))` semantic token. */
function mixShare(token: string) {
  const m = css.match(new RegExp(`--${token}:\\s*color-mix\\(in oklch, var\\(--p-([\\w-]+)\\) (\\d+)%, var\\(--p-([\\w-]+)\\)\\)`));
  if (!m) throw new Error(`No color-mix for ${token}`);
  return { from: m[1], share: +m[2] / 100, to: m[3] };
}

const combos = ['ink', 'satin'].flatMap((style) => ['light', 'dark'].map((theme) => [style, theme] as const));

describe.each(combos)('%s / %s', (style, theme) => {
  const p = primitives(style, theme);
  const window = toLinear(p.window);
  const solid = (name: string): Rgb => over(p[name], window);
  const surfaces: Record<string, Rgb> = {
    canvas: solid('canvas'),
    raised: solid('raised'),
    sunken: over(p.sunken, solid('canvas')),
    // The chrome is translucent over the window material; the window colour is its fallback.
    chrome: solid('chrome'),
  };
  const statusText = (status: 'success' | 'warning' | 'danger') => {
    const { from, share, to } = mixShare(`status-${status}-text`);
    return over(mixOklch(p[from], p[to], share), surfaces.canvas);
  };

  it.each(['text-1', 'text-2', 'text-3'])('%s reads at 4.5:1 on every surface', (text) => {
    for (const [name, surface] of Object.entries(surfaces)) {
      expect(contrast(over(p[text], surface), surface), `${text} on ${name}`).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('text on the accent reads at 4.5:1', () => {
    expect(contrast(over(p['on-accent'], solid('accent')), solid('accent'))).toBeGreaterThanOrEqual(4.5);
  });

  it('the accent stands out from the canvas at 3:1 (focus ring, badges, switches)', () => {
    expect(contrast(solid('accent'), surfaces.canvas)).toBeGreaterThanOrEqual(3);
  });

  it.each(['success', 'warning', 'danger'] as const)('%s text reads at 4.5:1 on the canvas and on its tint', (status) => {
    const text = statusText(status);
    const tint = over({ ...p[status], a: status === 'warning' ? 0.16 : 0.14 }, surfaces.canvas);
    expect(contrast(text, surfaces.canvas), 'on canvas').toBeGreaterThanOrEqual(4.5);
    expect(contrast(text, tint), 'on tint').toBeGreaterThanOrEqual(4.5);
  });

  it.each(['success', 'danger'] as const)('%s icons and dots stand out at 3:1', (status) => {
    expect(contrast(solid(status), surfaces.canvas)).toBeGreaterThanOrEqual(3);
  });

  it('an off switch track is distinguishable at 3:1', () => {
    expect(contrast(solid('switch-off'), surfaces.canvas)).toBeGreaterThanOrEqual(3);
  });
});
