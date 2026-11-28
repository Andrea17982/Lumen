import { useEffect, useState } from 'react';
import { Button, ConfirmDialog, Segmented, Select, Switch } from '../../design/components';
import { styles, type Appearance } from '../../design/appearance';
import type { ThemePreference } from '../../design/theme';
import { useT } from '../../i18n';
import { useSyncClient } from '../../sync/react';
import s from './Settings.module.css';

/** The daemon's settings as it stores them. The client changes only the fields it shows. */
export type AppSettings = Record<string, unknown> & { orchestra: boolean; codeMode: 'off' | 'auto' | 'on'; openAtLogin: boolean; closeToTray: boolean };
export type Language = 'system' | 'en' | 'it';

/** Reads the daemon's settings and saves a change merged into the latest copy. */
function useAppSettings() {
  const sync = useSyncClient();
  const [value, setValue] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    sync.call<AppSettings>('get_settings').then(setValue, (e) => setError(String(e)));
  }, [sync]);
  async function run(command: string, change?: Partial<AppSettings>) {
    setSaved(false);
    setError(null);
    try {
      const latest = await sync.call<AppSettings>('get_settings');
      setValue(await sync.call<AppSettings>(command, change ? { value: { ...latest, ...change } } : {}));
      setSaved(true);
    } catch (e) {
      setError(String(e));
    }
  }
  return { value, error, saved, save: (change: Partial<AppSettings>) => run('save_settings', change), reset: () => run('reset_settings') };
}

export interface Preferences {
  appearance: Appearance;
  setAppearance: (next: Partial<Appearance>) => void;
  language: Language;
  setLanguage: (next: Language) => void;
  taskActions: boolean;
  setTaskActions: (on: boolean) => void;
}

export function AppearanceFields({ preferences }: { preferences: Pick<Preferences, 'appearance' | 'setAppearance' | 'language' | 'setLanguage'> }) {
  const t = useT();
  const { appearance, setAppearance, language, setLanguage } = preferences;
  return (
    <>
      <Select label={t.languageLabel} value={language} options={[{ id: 'system', label: t.general.systemLanguage }, { id: 'en', label: t.language.en }, { id: 'it', label: t.language.it }]} onChange={setLanguage} />
      <Segmented label={t.styleLabel} value={appearance.style} options={styles.map((id) => ({ id, label: t.styles[id] }))} onChange={(style) => setAppearance({ style })} />
      <Segmented<ThemePreference> label={t.themeLabel} value={appearance.theme} options={(['system', 'light', 'dark'] as const).map((id) => ({ id, label: t.theme[id] }))} onChange={(theme) => setAppearance({ theme })} />
    </>
  );
}

/**
 * The few choices Lumen asks for. Everything else has a default that just works; the daemon keeps
 * the rest of its settings as they are.
 */
export function General({ preferences }: { preferences: Preferences }) {
  const t = useT();
  const { value, error, saved, save, reset } = useAppSettings();
  return (
    <div className={s.section}>
      <div className={s.option}>
        <div className={s.fields}>
          <AppearanceFields preferences={preferences} />
        </div>
        <Switch isSelected={preferences.taskActions} onChange={preferences.setTaskActions}>
          {t.general.taskActions}
        </Switch>
      </div>
      {value && (
        <>
          <div className={s.option} data-tour-target="orchestra">
            <Switch isSelected={value.orchestra} onChange={(orchestra) => void save({ orchestra })}>
              <span className={s.switchText}>
                {t.general.orchestra}
                <small>{t.general.orchestraHelp}</small>
              </span>
            </Switch>
            <Switch isSelected={value.codeMode !== 'off'} onChange={(on) => void save({ codeMode: on ? 'auto' : 'off', codeModeChosen: true })}>
              <span className={s.switchText}>
                {t.general.codeMode}
                <small>{t.general.codeModeHelp}</small>
              </span>
            </Switch>
          </div>
          <div className={s.option}>
            <Switch isSelected={value.openAtLogin} onChange={(openAtLogin) => void save({ openAtLogin })}>
              {t.general.openAtLogin}
            </Switch>
            <Switch isSelected={value.closeToTray} onChange={(closeToTray) => void save({ closeToTray })}>
              <span className={s.switchText}>
                {t.general.tray}
                <small>{t.general.trayHelp}</small>
              </span>
            </Switch>
          </div>
          <div>
            <ConfirmDialog trigger={<Button variant="plain">{t.general.reset}</Button>} title={t.general.resetTitle} body={t.general.resetBody} confirmLabel={t.general.reset} cancelLabel={t.cancel} onConfirm={() => void reset()} />
          </div>
        </>
      )}
      <p className={s.status} role="status">
        {saved ? t.general.saved : ''}
      </p>
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
      <About />
    </div>
  );
}

const AGENTS: [string, string][] = [
  ['claude', 'Claude Code'],
  ['gpt', 'Codex'],
  ['antigravity', 'Antigravity'],
  ['ollama', 'Ollama'],
];

function About() {
  const t = useT();
  const sync = useSyncClient();
  const [status, setStatus] = useState<{ version: string } | null>(null);
  const [found, setFound] = useState<Record<string, unknown> | null>(null);
  useEffect(() => {
    sync.call<{ version: string }>('__status').then(setStatus, () => {});
    sync.call<Record<string, unknown>>('diagnostics').then(setFound, () => {});
  }, [sync]);
  const agents = AGENTS.filter(([id]) => found?.[id] === true).map(([, name]) => name);
  return (
    <details className={s.about}>
      <summary>{t.general.about}</summary>
      <dl>
        {status && (
          <div>
            <dt>{t.general.version(status.version)}</dt>
          </div>
        )}
        {found && (
          <>
            <div>
              <dt>{t.general.found}</dt>
              <dd>{agents.length ? agents.join(', ') : t.general.none}</dd>
            </div>
            <div>
              <dt>{t.general.dataFolder}</dt>
              <dd>
                <code>{String(found.dataDir ?? '')}</code>
              </dd>
            </div>
          </>
        )}
      </dl>
    </details>
  );
}
