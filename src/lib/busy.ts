import { useCallback, useState } from 'react';
import { saveSettings, useAppSettings } from './settings';
import type { AppSettings } from './types';
import { errorText } from './util';

/** `busy` while `run` awaits its task; a failure goes to `onError` as readable text. */
export function useBusy(onError: (text: string) => void) {
  const [busy, setBusy] = useState(false);
  const run = useCallback(
    async (task: () => Promise<unknown>) => {
      setBusy(true);
      try {
        await task();
      } catch (e) {
        onError(errorText(e));
      } finally {
        setBusy(false);
      }
    },
    [onError],
  );
  return [busy, run] as const;
}

/** The shared backend settings plus a guarded `save`, for the Settings tabs. */
export function useSettingsEditor(onError: (text: string) => void) {
  const s = useAppSettings(onError);
  const [busy, run] = useBusy(onError);
  const save = (next: AppSettings) => void run(() => saveSettings(next));
  return { s, busy, run, save };
}
