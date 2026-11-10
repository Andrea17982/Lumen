import { useEffect, useState } from 'react';
import { call } from '../lib/bridge';
import { useI18n } from '../lib/i18n';
import { errorText } from '../lib/util';
import type { GodotDiagnostics, Screenshot } from '../lib/types';
import ResponseImage from './ResponseImage';
interface Settings {
  enabled: boolean;
  server: string;
}
interface Result {
  available: string[];
  detail: string;
  screenshot: Screenshot | null;
}
export default function GameControls({
  folder,
  onInspect,
}: {
  folder: string;
  onInspect: (file: string) => void;
}) {
  const { t } = useI18n();
  const [settings, setSettings] = useState<Settings>({ enabled: false, server: '' });
  const [servers, setServers] = useState<string[]>([]);
  const [scene, setScene] = useState('');
  const [result, setResult] = useState<Result | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    let active = true;
    void Promise.all([
      call<Settings>('godot_visual_settings', { folder }),
      call<GodotDiagnostics>('project_godot_diagnostics', { folder }),
    ])
      .then(([s, g]) => {
        if (active) {
          setSettings(s);
          setServers(g.servers);
        }
      })
      .catch((e) => {
        if (active) setError(errorText(e));
      });
    return () => {
      active = false;
    };
  }, [folder]);
  async function save(value: Settings) {
    setBusy(true);
    setError('');
    setResult(null);
    try {
      setSettings(await call<Settings>('set_godot_visual_settings', { folder, value }));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }
  async function action(action: string) {
    setBusy(true);
    setError('');
    try {
      setResult(await call<Result>('godot_visual_action', { folder, action, scene }));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section aria-label={t('Godot visual controls')}>
      <p className="panel-hint">
        {t(
          'Controls are off until enabled for this project. Every action verifies that the connected editor belongs to this folder. No input is automated.',
        )}
      </p>
      <label className="game-opt-in">
        <input
          type="checkbox"
          checked={settings.enabled}
          disabled={busy || !servers.length}
          onChange={(e) =>
            void save({
              ...settings,
              enabled: e.target.checked,
              server: settings.server || servers[0] || '',
            })
          }
        />
        {t('Enable visual controls for this project')}
      </label>
      {servers.length ? (
        <label className="native-custom">
          {t('Godot MCP server')}
          <select
            value={settings.server || servers[0]}
            disabled={busy}
            onChange={(e) => void save({ ...settings, server: e.target.value })}
          >
            {servers.map((s) => (
              <option key={s}>{s}</option>
            ))}
          </select>
        </label>
      ) : (
        <p>
          {t(
            'Declare a project Godot MCP server to use Play and Reload. Window capture remains available in Visual review.',
          )}
        </p>
      )}
      {settings.enabled && (
        <>
          <button disabled={busy} onClick={() => void action('discover')}>
            {t('Connect and check actions')}
          </button>
          {result && <p role="status">{result.detail}</p>}
          <label className="native-custom">
            {t('Scene path (empty uses main scene)')}
            <input
              placeholder="res://scenes/main.tscn"
              value={scene}
              onChange={(e) => setScene(e.target.value)}
            />
          </label>
          <div className="game-actions">
            {(['play', 'reload', 'stop', 'capture'] as const).map((id) => (
              <button
                key={id}
                disabled={busy || !result?.available.includes(id)}
                onClick={() => void action(id)}
              >
                {t(
                  {
                    play: 'Play scene',
                    reload: 'Reload project',
                    stop: 'Stop scene',
                    capture: 'Capture game viewport',
                  }[id],
                )}
              </button>
            ))}
          </div>
        </>
      )}
      {busy && <p role="status">{t('Waiting for Godot…')}</p>}
      {error && <p role="alert">{error}</p>}
      {result?.screenshot && (
        <>
          <ResponseImage
            href={result.screenshot.path}
            alt={result.screenshot.label}
            folder={folder}
          />
          <button onClick={() => onInspect(result.screenshot!.path)}>
            {t('Inspect pixels and grid')}
          </button>
        </>
      )}
    </section>
  );
}
