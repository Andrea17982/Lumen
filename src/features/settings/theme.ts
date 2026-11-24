import { useEffect, useState } from 'react';
import type { Contributions } from '../../sync/sync';

const KEY = 'lumen.pluginTheme';
const listeners = new Set<(value: string | null) => void>();

function read(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

/** The chosen plugin theme, `plugin/theme`, shared by every component that shows it. */
export function usePluginTheme() {
  const [value, setValue] = useState(read);
  useEffect(() => {
    listeners.add(setValue);
    return () => void listeners.delete(setValue);
  }, []);
  const choose = (next: string | null) => {
    try {
      if (next) localStorage.setItem(KEY, next);
      else localStorage.removeItem(KEY);
    } catch {
      // Storage can be unavailable; the choice then lasts for this window only.
    }
    for (const listener of listeners) listener(next);
  };
  return [value, choose] as const;
}

const SCHEME: Record<string, string> = { any: ':root', light: ":root[data-theme='light']", dark: ":root[data-theme='dark']" };
const NAME = /^--[a-z0-9-]{1,64}$/i;
// Status colours and the focus ring stay Lumen's: a theme must not make Reject look like Approve.
const KEPT = /^--(status-|focus-|text-on-accent|p-danger|p-success|p-warning|p-on-accent)/i;
const UNSAFE = /[{}<>\\@"';]|url\(|expression|image-set|src\(/i;

/**
 * CSS for a plugin theme. The daemon already accepts only tokens; this re-checks each one, so a
 * token can never close the block or load anything.
 */
export function themeCss(theme: Contributions['themes'][number]): string {
  return Object.entries(theme.tokens)
    .filter(([scheme]) => scheme in SCHEME)
    .map(([scheme, tokens]) => {
      const body = Object.entries(tokens)
        .filter(([name, value]) => NAME.test(name) && !KEPT.test(name) && value.length <= 200 && !UNSAFE.test(value) && !value.includes('/*'))
        .map(([name, value]) => `${name}: ${value};`)
        .join(' ');
      return `${SCHEME[scheme]} { ${body} }`;
    })
    .join('\n');
}
