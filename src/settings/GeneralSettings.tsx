import { useEffect, useState } from 'react';
import { Monitor, Moon, RotateCcw, Sun } from 'lucide-react';
import { call } from '../lib/bridge';
import { saveSettings, useAppSettings } from '../lib/settings';
import type { AppSettings } from '../lib/types';
import { errorText } from '../lib/util';
import { Switch } from './controls';
import { useI18n } from '../lib/i18n';

/** Settings → General: interface and window behavior. */
export default function GeneralSettings({
  theme,
  setTheme,
  onError,
  setAutoPolicy,
  setBalanceUsage,
}: {
  theme: string;
  setTheme: (t: string) => void;
  onError: (s: string) => void;
  setAutoPolicy: (value: string) => void;
  setBalanceUsage: (value: boolean) => void;
}) {
  const { language, setLanguage, t } = useI18n();
  const s = useAppSettings(onError);
  const [busy, setBusy] = useState(false);
  const [confirmReset, setConfirmReset] = useState(false);
  const [largerText, setLargerText] = useState(
    () => localStorage.getItem('lumen-larger-text') === 'true',
  );
  useEffect(() => {
    document.documentElement.dataset.textSize = largerText ? 'large' : 'default';
  }, [largerText]);
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
  async function reset() {
    setBusy(true);
    try {
      await call<AppSettings>('reset_settings');
      localStorage.setItem('lumen-theme', 'system');
      localStorage.setItem('lumen-larger-text', 'false');
      localStorage.setItem('lumen-auto-policy', 'default');
      localStorage.setItem('lumen-balance-usage', 'false');
      setTheme('system');
      setLanguage('en');
      setAutoPolicy('default');
      setBalanceUsage(false);
      setLargerText(false);
      setConfirmReset(false);
    } catch (e) {
      onError(errorText(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <section className="settings-section">
        <h3>{t('Interface')}</h3>
        <label className="language-control">
          <span>{t('Language')}</span>
          <select
            aria-label={t('Language')}
            value={language}
            onChange={(event) => setLanguage(event.target.value as 'en' | 'it')}
          >
            <option value="en">English</option>
            <option value="it">Italiano</option>
          </select>
        </label>
        <div className="theme-switch" role="group" aria-label={t('Theme')}>
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
              {t(id.charAt(0).toUpperCase() + id.slice(1))}
            </button>
          ))}
        </div>
        <Switch
          label={t('Larger text')}
          hint={t('Increase interface text size while keeping the current layout style.')}
          checked={largerText}
          onChange={(value) => {
            setLargerText(value);
            localStorage.setItem('lumen-larger-text', String(value));
          }}
        />
      </section>
      {s && (
        <section className="settings-section">
          <h3>{t('Window')}</h3>
          <Switch
            label={t('Keep Lumen in the notification area when closed')}
            hint={t('Closing the window keeps running responses going. Quit from the tray icon.')}
            checked={s.closeToTray}
            disabled={busy}
            onChange={(v) => void save({ ...s, closeToTray: v })}
          />
          <Switch
            label={t('Open Lumen at login')}
            hint={
              s.closeToTray
                ? t('Starts quietly in the notification area.')
                : t('Opens the window when you sign in.')
            }
            checked={s.openAtLogin}
            disabled={busy}
            onChange={(v) => void save({ ...s, openAtLogin: v })}
          />
        </section>
      )}
      <section className="settings-section settings-reset">
        <h3>{t('Restore defaults')}</h3>
        <p>
          {t(
            'Reset Lumen preferences in Simple and Advanced settings. Accounts and conversations are kept.',
          )}
        </p>
        {confirmReset ? (
          <div className="auto-policy">
            <button disabled={busy} onClick={() => setConfirmReset(false)}>
              {t('Cancel')}
            </button>
            <button disabled={busy} onClick={() => void reset()}>
              {t('Confirm reset')}
            </button>
          </div>
        ) : (
          <button className="text-button" disabled={busy} onClick={() => setConfirmReset(true)}>
            <RotateCcw size={14} /> {t('Restore all settings to defaults')}
          </button>
        )}
      </section>
    </>
  );
}
