import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { call, desktop } from './bridge';
import type { Screenshot } from './types';
import { errorText } from './util';
export function useCaptureHotkey(
  folder: string,
  attach: (path: string) => void,
  onError: (message: string) => void,
) {
  const capturing = useRef(false);
  useEffect(() => {
    if (!desktop) return;
    let active = true;
    let dispose = () => {};
    void listen('lumen:capture-hotkey', () => {
      if (capturing.current) return;
      if (!folder) {
        onError('Choose a project folder before capturing a screenshot.');
        return;
      }
      capturing.current = true;
      void call<Screenshot>('capture_screenshot', {
        folder,
        window: null,
        label: 'Composer capture',
      })
        .then((shot) => {
          if (active) attach(shot.path);
        })
        .catch((e) => {
          if (active) onError(errorText(e));
        })
        .finally(() => {
          capturing.current = false;
        });
    })
      .then((off) => {
        if (active) dispose = off;
        else off();
      })
      .catch((e) => onError(errorText(e)));
    return () => {
      active = false;
      dispose();
    };
  }, [folder, attach, onError]);
}
