import { useState } from 'react';
import { ArrowUpRight, Folder, Monitor, Moon, Sun } from 'lucide-react';
import { saveSettings, useAppSettings } from '../lib/settings';
import type { AppSettings } from '../lib/types';
import { errorText } from '../lib/util';
import { Switch } from './controls';

/** Settings → General: appearance, default folder and window behavior. */
export default function GeneralSettings({
  theme,
  setTheme,
  workspace,
  chooseFolder,
  onError,
}: {
  theme: string;
  setTheme: (t: string) => void;
  workspace: string;
  chooseFolder: () => Promise<unknown>;
  onError: (s: string) => void;
}) {
  const s = useAppSettings(onError);
  const [busy, setBusy] = useState(false);
  async function save(next: AppSettings) {
    setBusy(true);
    try {
      await saveSettings(next);
    } catch (e) {
      onError(errorText(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <section className="settings-section">
        <h3>Appearance</h3>
        <div className="theme-switch" role="group" aria-label="Theme">
          {[
            { id: 'light', Icon: Sun },
            { id: 'dark', Icon: Moon },
            { id: 'system', Icon: Monitor },
          ].map(({ id, Icon }) => (
            <button
              key={id}
              aria-pressed={theme === id}
              className={theme === id ? 'active' : ''}
              onClick={() => setTheme(id)}
            >
              <Icon size={16} />
              {id.charAt(0).toUpperCase() + id.slice(1)}
            </button>
          ))}
        </div>
      </section>
      <section className="settings-section">
        <h3>Conversation folder</h3>
        <p>Linked to this conversation. New conversations use your last selection.</p>
        <button
          className="folder-button"
          onClick={() => void chooseFolder().catch((e) => onError(errorText(e)))}
        >
          <Folder size={16} />
          <span>{workspace || 'Choose a folder'}</span>
          <ArrowUpRight size={14} />
        </button>
      </section>
      {s && (
        <section className="settings-section">
          <h3>Window</h3>
          <Switch
            label="Keep Lumen in the notification area when closed"
            hint="Closing the window keeps running responses going. Quit from the tray icon."
            checked={s.closeToTray}
            disabled={busy}
            onChange={(v) => void save({ ...s, closeToTray: v })}
          />
          <Switch
            label="Open Lumen at login"
            hint={
              s.closeToTray
                ? 'Starts quietly in the notification area.'
                : 'Opens the window when you sign in.'
            }
            checked={s.openAtLogin}
            disabled={busy}
            onChange={(v) => void save({ ...s, openAtLogin: v })}
          />
        </section>
      )}
    </>
  );
}
