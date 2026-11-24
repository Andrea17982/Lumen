import { useEffect, useState } from 'react';

export type ThemePreference = 'system' | 'light' | 'dark';
export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const dark = () => matchMedia('(prefers-color-scheme: dark)').matches;

/** Resolves the preference to light/dark on <html> and keeps the native window material in step. */
export function useTheme(preference: ThemePreference) {
  const [systemDark, setSystemDark] = useState(dark);
  useEffect(() => {
    const query = matchMedia('(prefers-color-scheme: dark)');
    const change = () => setSystemDark(query.matches);
    query.addEventListener('change', change);
    return () => query.removeEventListener('change', change);
  }, []);
  const resolved = preference === 'system' ? (systemDark ? 'dark' : 'light') : preference;
  useEffect(() => {
    document.documentElement.dataset.theme = resolved;
    if (isTauri) document.documentElement.dataset.host = 'tauri';
  }, [resolved]);
  useEffect(() => {
    if (!isTauri) return;
    // Mica and Liquid Glass follow the window theme, not the page.
    import('@tauri-apps/api/window').then(({ getCurrentWindow }) =>
      getCurrentWindow().setTheme(preference === 'system' ? null : preference),
    );
  }, [preference]);
  return resolved;
}
