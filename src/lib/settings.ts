import { useEffect, useState } from 'react';
import { call } from './bridge';
import type { AppSettings } from './types';

/** One shared copy of the backend settings, so Settings and the composer stay in sync. */
let current: AppSettings | null = null;
let loading: Promise<AppSettings> | null = null;
const listeners = new Set<(s: AppSettings) => void>();
const publish = (v: AppSettings) => {
  current = v;
  listeners.forEach((l) => l(v));
  return v;
};

export function loadSettings() {
  loading ??= call<AppSettings>('get_settings')
    .then(publish)
    .catch((e) => {
      loading = null;
      throw e;
    });
  return loading;
}
export async function saveSettings(next: AppSettings) {
  return publish(await call<AppSettings>('save_settings', { value: next }));
}
export function useAppSettings(onError?: (s: string) => void) {
  const [s, setS] = useState(current);
  useEffect(() => {
    listeners.add(setS);
    if (!current) void loadSettings().catch((e) => onError?.(String(e)));
    return () => void listeners.delete(setS);
  }, [onError]);
  return s;
}

/** Advanced options that differ from their defaults; Simple mode mentions them. */
export function advancedActive(s: AppSettings | null, balanceUsage: boolean, mcpOn = 0) {
  if (!s) return mcpOn;
  return (
    mcpOn +
    [
      balanceUsage,
      !s.selfHeal,
      s.reviewRounds !== 1,
      s.apiEnabled,
      Boolean(s.ollamaModel),
      !s.autoFallback,
      !s.repoContext,
    ].filter(Boolean).length
  );
}
