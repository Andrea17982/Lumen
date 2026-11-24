import { describe, expect, it } from 'vitest';
import { en } from './en';
import { it as itCatalog } from './it';
import { plural } from './plural';
import { formatNumber, formatPercent, formatRelative } from './format';

type Leaf = string | ((...args: never[]) => string);
function leaves(obj: object, prefix = ''): [string, Leaf][] {
  return Object.entries(obj).flatMap(([k, v]) =>
    typeof v === 'object' && v !== null ? leaves(v, `${prefix}${k}.`) : [[`${prefix}${k}`, v as Leaf] as [string, Leaf]],
  );
}
// Sample arguments for parameterised messages, by name.
const sample = (fn: (...args: never[]) => string) => (fn as (...a: unknown[]) => string)(fn.length ? 3 : undefined, 'x');
const render = (v: Leaf) => (typeof v === 'function' ? sample(v) : v);

// Words that are the same in both languages on purpose.
const sameInBoth = new Set(['appName', 'modeAuto', 'styles.ink', 'styles.satin', 'language.en', 'language.it', 'kinds.skill', 'kinds.browser', 'general.orchestra', 'orchestra.title']);

describe('catalogs', () => {
  const english = new Map(leaves(en));
  const italian = new Map(leaves(itCatalog));

  it('have the same keys', () => {
    expect([...italian.keys()].sort()).toEqual([...english.keys()].sort());
  });

  it('have no empty messages', () => {
    for (const [key, value] of [...english, ...italian]) expect(render(value).trim(), key).not.toBe('');
  });

  it('are translated, except words shared by both languages', () => {
    const untranslated = [...english].filter(([k, v]) => !sameInBoth.has(k) && render(v) === render(italian.get(k)!)).map(([k]) => k);
    expect(untranslated).toEqual([]);
  });

  it('use typographic apostrophes and ellipses in Italian', () => {
    for (const [key, value] of italian) {
      expect(render(value), key).not.toMatch(/\w'\w/);
      expect(render(value), key).not.toContain('...');
    }
  });

  it('pluralise', () => {
    expect(en.editedFiles(1)).toBe('Edited 1 file');
    expect(en.editedFiles(3)).toBe('Edited 3 files');
    expect(itCatalog.editedFiles(1)).toBe('Modificato 1 file');
    expect(itCatalog.checksPassed(2)).toBe('2 verifiche superate');
    expect(plural(0, 'it', { one: 'uno', other: 'altri' })).toBe('altri');
  });
});

describe('formats', () => {
  it('show absent values as absent, never as zero', () => {
    expect(formatPercent('it', null)).toBe('—');
    expect(formatNumber('en', undefined)).toBe('—');
    expect(formatPercent('en', 0)).toBe('0%');
  });

  it('follow the locale', () => {
    expect(formatNumber('it', 1234.5)).toBe('1234,5');
    expect(formatNumber('en', 1234.5)).toBe('1,234.5');
    expect(formatPercent('it', 72.4)).toBe('72%');
  });

  it('say relative times in words', () => {
    const now = Date.UTC(2026, 10, 24, 12);
    expect(formatRelative('en', now + 3 * 3600_000, now)).toBe('in 3 hours');
    expect(formatRelative('it', now + 3 * 3600_000, now)).toBe('tra 3 ore');
    expect(formatRelative('it', now - 5 * 60_000, now)).toBe('5 minuti fa');
    expect(formatRelative('en', now - 86400_000, now)).toBe('yesterday');
  });
});
