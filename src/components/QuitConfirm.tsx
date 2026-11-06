import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { call, desktop } from '../lib/bridge';

/** Shown when the window is closed while a response is still running (tray off). */
export default function QuitConfirm() {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!desktop) return;
    const off = listen('lumen:confirm-quit', () => setOpen(true));
    return () => void off.then((f) => f());
  }, []);
  if (!open) return null;
  return (
    <div className="quit-confirm" role="alertdialog" aria-label="Quit Lumen">
      <p>A response is still running. Quit anyway?</p>
      <div>
        <button className="text-button" autoFocus onClick={() => setOpen(false)}>
          Keep running
        </button>
        <button className="primary-button" onClick={() => void call('quit_app')}>
          Quit
        </button>
      </div>
    </div>
  );
}
