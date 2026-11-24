import { useEffect, useState } from 'react';
import { Switch } from '../../design/components';
import { useT } from '../../i18n';
import { useSyncClient } from '../../sync/react';
import s from './Settings.module.css';

/** The daemon's settings as it stores them. The client changes only the fields it shows. */
export type AppSettings = Record<string, unknown> & { orchestra: boolean };

/** Reads the daemon's settings and saves a change merged into the latest copy. */
export function useAppSettings() {
  const sync = useSyncClient();
  const [value, setValue] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    sync.call<AppSettings>('get_settings').then(setValue, (e) => setError(String(e)));
  }, [sync]);
  async function save(change: Partial<AppSettings>) {
    setSaved(false);
    setError(null);
    try {
      const latest = await sync.call<AppSettings>('get_settings');
      setValue(await sync.call<AppSettings>('save_settings', { value: { ...latest, ...change } }));
      setSaved(true);
    } catch (e) {
      setError(String(e));
    }
  }
  return { value, error, saved, save };
}

export function General() {
  const t = useT();
  const { value, error, saved, save } = useAppSettings();
  if (!value) return error ? <p className={s.error}>{error}</p> : null;
  return (
    <div className={s.section}>
      <div className={s.option}>
        <Switch isSelected={value.orchestra} onChange={(orchestra) => void save({ orchestra })}>
          <span className={s.switchText}>
            {t.general.orchestra}
            <small>{t.general.orchestraHelp}</small>
          </span>
        </Switch>
      </div>
      <p className={s.status} role="status">
        {saved ? t.general.saved : ''}
      </p>
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
