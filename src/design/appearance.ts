import { useEffect, useState } from 'react';
import { useTheme, type ThemePreference } from './theme';

/** Ink is Lumen's look; Satin is the optional, more classic alternative in Settings. */
export type Style = 'ink' | 'satin';
export const styles: Style[] = ['ink', 'satin'];

export interface Appearance {
  style: Style;
  theme: ThemePreference;
}
const KEY = 'lumen.appearance';
const fallback: Appearance = { style: 'ink', theme: 'system' };

// Presentation-only preference, so it stays in the client until the daemon stores client settings.
function load(): Appearance {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? 'null') as Partial<Appearance> | null;
    return {
      style: saved?.style && styles.includes(saved.style) ? saved.style : fallback.style,
      theme: saved?.theme && ['system', 'light', 'dark'].includes(saved.theme) ? saved.theme : fallback.theme,
    };
  } catch {
    return fallback;
  }
}

export function useAppearance() {
  const [appearance, setAppearance] = useState(load);
  useTheme(appearance.theme);
  useEffect(() => {
    document.documentElement.dataset.style = appearance.style;
    try {
      localStorage.setItem(KEY, JSON.stringify(appearance));
    } catch {
      // Storage can be unavailable (private mode, blocked site data); the choice then lasts the session.
    }
  }, [appearance]);
  return [appearance, (next: Partial<Appearance>) => setAppearance((a) => ({ ...a, ...next }))] as const;
}
