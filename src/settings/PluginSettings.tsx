import { useCallback, useEffect, useRef, useState } from 'react';
import { call, desktop } from '../lib/bridge';
import { useBusy } from '../lib/busy';
import { useI18n } from '../lib/i18n';
import {
  INSERT_PROMPT,
  PANEL_SANDBOX,
  PLUGINS_CHANGED,
  THEME_KEY,
  handlePanelMessage,
  panelUrl,
  type PluginPanel,
} from '../lib/plugins';
import { usePluginContributions } from '../lib/usePlugins';
import { errorText } from '../lib/util';

interface Manifest {
  id: string;
  name: string;
  version: string;
  description: string;
  permissions: { secrets: string[] };
}
interface Installed {
  id: string;
  manifest?: Manifest;
  enabled: boolean;
  source: string;
  grants?: string[];
  pending?: string[];
  error?: string;
}
interface Review {
  token: string;
  manifest: Manifest;
  grants: string[];
  added: string[];
  update: boolean;
  installedVersion?: string;
  applied?: boolean;
}

function Grants({ items, empty }: { items: string[]; empty: string }) {
  return items.length ? (
    <ul className="plugin-grants">
      {items.map((g) => (
        <li key={g}>{g}</li>
      ))}
    </ul>
  ) : (
    <small>{empty}</small>
  );
}

/** A sandboxed panel: opaque origin, its own strict CSP, and only the postMessage broker. */
export function PluginPanelFrame({ panel }: { panel: PluginPanel }) {
  const frame = useRef<HTMLIFrameElement>(null);
  useEffect(() => {
    const listen = (event: MessageEvent) => {
      const source = frame.current?.contentWindow;
      if (!source || event.source !== source) return;
      void handlePanelMessage(event.data, panel, {
        insertPrompt: (text) =>
          window.dispatchEvent(new CustomEvent(INSERT_PROMPT, { detail: text })),
        fetch: (url) => call('plugin_panel_fetch', { id: panel.plugin, url }),
      }).then((reply) => reply && source.postMessage(reply, '*'));
    };
    window.addEventListener('message', listen);
    return () => window.removeEventListener('message', listen);
  }, [panel]);
  return (
    <iframe
      ref={frame}
      className="plugin-panel"
      title={panel.title}
      sandbox={PANEL_SANDBOX}
      referrerPolicy="no-referrer"
      src={panelUrl(panel.plugin, panel.id)}
    />
  );
}

/** Settings → Plugins: install with a permission review, enable, reload, remove. */
export default function PluginSettings({ onError }: { onError: (s: string) => void }) {
  const { t } = useI18n();
  const [plugins, setPlugins] = useState<Installed[]>([]);
  const [source, setSource] = useState('');
  const [review, setReview] = useState<Review | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);
  const [openPanel, setOpenPanel] = useState<string | null>(null);
  const [theme, setTheme] = useState(() => localStorage.getItem(THEME_KEY) || '');
  const [busy, run] = useBusy(onError);
  const contributions = usePluginContributions();
  const refresh = useCallback(async () => {
    setPlugins(await call<Installed[]>('plugins_list'));
    window.dispatchEvent(new Event(PLUGINS_CHANGED));
  }, []);
  useEffect(() => {
    if (desktop) void refresh().catch((e) => onError(errorText(e)));
  }, [onError, refresh]);
  const act = (fn: () => Promise<unknown>) =>
    run(async () => {
      await fn();
      await refresh();
    });
  const inspect = (from: string) =>
    run(async () => setReview(await call<Review>('plugin_inspect', { source: from })));
  const chooseTheme = (key: string) => {
    setTheme(key);
    if (key) localStorage.setItem(THEME_KEY, key);
    else localStorage.removeItem(THEME_KEY);
    window.dispatchEvent(new Event(PLUGINS_CHANGED));
  };
  return (
    <section className="settings-section">
      <div className="section-title">
        <h3>{t('Plugins')}</h3>
      </div>
      <p>
        {t(
          'Plugins add themes, slash commands, skills, tools and panels. Each one asks only for what it declares, and stays off until you turn it on.',
        )}
      </p>
      <form
        className="account-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (source.trim()) void inspect(source.trim());
        }}
      >
        <label>
          {t('Folder or https:// git URL')}
          <input
            value={source}
            placeholder="https://github.com/you/lumen-plugin.git"
            onChange={(e) => setSource(e.target.value)}
          />
        </label>
        <div className="panel-row">
          <button className="primary-button" disabled={!desktop || busy || !source.trim()}>
            {t('Review')}
          </button>
          <button
            type="button"
            className="text-button"
            disabled={!desktop || busy}
            onClick={() =>
              void run(async () => {
                const { open } = await import('@tauri-apps/plugin-dialog');
                const path = await open({
                  directory: true,
                  multiple: false,
                  title: t('Plugin folder'),
                });
                if (typeof path === 'string') {
                  setSource(path);
                  await inspect(path);
                }
              })
            }
          >
            {t('Choose folder')}
          </button>
        </div>
      </form>
      {review && (
        <div className="plugin-review" role="group" aria-label={t('Plugin review')}>
          <strong>
            {review.manifest.name} {review.manifest.version}
            {review.update &&
              review.installedVersion &&
              ` (${t('installed')} ${review.installedVersion})`}
          </strong>
          {review.manifest.description && <small>{review.manifest.description}</small>}
          <span>
            {review.update ? t('New permissions in this update') : t('This plugin asks to')}:
          </span>
          <Grants
            items={review.update ? review.added : review.grants}
            empty={review.update ? t('Nothing new.') : t('Nothing beyond its own files.')}
          />
          {review.update && review.added.length > 0 && (
            <small>{t('The plugin stays off until you turn it on again.')}</small>
          )}
          <div className="panel-row">
            <button
              className="primary-button"
              disabled={busy}
              onClick={() =>
                void act(async () => {
                  await call('plugin_install', { token: review.token });
                  setReview(null);
                  setSource('');
                })
              }
            >
              {review.update ? t('Update') : t('Install (off)')}
            </button>
            <button className="text-button" onClick={() => setReview(null)}>
              {t('Cancel')}
            </button>
          </div>
        </div>
      )}
      {plugins.length === 0 && !review && (
        <div className="account-empty">{t('No plugins yet.')}</div>
      )}
      {plugins.map((p) => (
        <div className="plugin-row" key={p.id}>
          <div className="panel-row">
            <span>
              <strong>{p.manifest?.name || p.id}</strong> <small>{p.manifest?.version}</small>
            </span>
            <label className="plugin-toggle">
              <input
                type="checkbox"
                role="switch"
                className="switch"
                aria-label={`${t('Enable')} ${p.manifest?.name || p.id}`}
                checked={p.enabled}
                disabled={busy || !!p.error}
                onChange={(e) =>
                  e.target.checked
                    ? setConfirming(p.id)
                    : void act(() => call('plugin_set_enabled', { id: p.id, on: false }))
                }
              />
            </label>
          </div>
          {p.error && <small className="plugin-error">{p.error}</small>}
          {!!p.pending?.length && (
            <small className="plugin-error">
              {t('Changed on disk and now asks for more; turn it on again to approve.')}
            </small>
          )}
          {confirming === p.id && (
            <div className="plugin-review" role="group" aria-label={t('Approve permissions')}>
              <span>{t('Turning it on allows it to')}:</span>
              <Grants items={p.grants || []} empty={t('Nothing beyond its own files.')} />
              <div className="panel-row">
                <button
                  className="primary-button"
                  onClick={() =>
                    void act(async () => {
                      await call('plugin_set_enabled', { id: p.id, on: true });
                      setConfirming(null);
                    })
                  }
                >
                  {t('Turn on')}
                </button>
                <button className="text-button" onClick={() => setConfirming(null)}>
                  {t('Cancel')}
                </button>
              </div>
            </div>
          )}
          {p.enabled &&
            p.manifest?.permissions.secrets.map((name) => (
              <form
                key={name}
                className="panel-row"
                onSubmit={(e) => {
                  e.preventDefault();
                  const field = e.currentTarget.elements.namedItem('value') as HTMLInputElement;
                  void act(async () => {
                    await call('plugin_set_secret', { id: p.id, name, value: field.value });
                    field.value = '';
                  });
                }}
              >
                <input
                  name="value"
                  type="password"
                  aria-label={name}
                  placeholder={name}
                  autoComplete="off"
                />
                <button className="text-button">{t('Save secret')}</button>
              </form>
            ))}
          {contributions.themes
            .filter((th) => th.plugin === p.id)
            .map((th) => {
              const key = `${th.plugin}/${th.id}`;
              return (
                <button
                  key={key}
                  className="text-button"
                  aria-pressed={theme === key}
                  onClick={() => chooseTheme(theme === key ? '' : key)}
                >
                  {theme === key ? t('Stop using') : t('Use theme')} {th.name}
                </button>
              );
            })}
          {contributions.panels
            .filter((panel) => panel.plugin === p.id)
            .map((panel) => {
              const key = `${panel.plugin}/${panel.id}`;
              return (
                <div key={key}>
                  <button
                    className="text-button"
                    onClick={() => setOpenPanel(openPanel === key ? null : key)}
                  >
                    {openPanel === key ? t('Close') : t('Open')} {panel.title}
                  </button>
                  {openPanel === key && <PluginPanelFrame panel={panel} />}
                </div>
              );
            })}
          <div className="panel-row">
            <button
              className="text-button"
              disabled={busy || !p.source}
              onClick={() =>
                void run(async () => {
                  const next = await call<Review>('plugin_reload', { id: p.id });
                  if (!next.applied) setReview(next);
                  await refresh();
                })
              }
            >
              {t('Reload')}
            </button>
            <button
              className="text-button"
              disabled={busy}
              onClick={() => void act(() => call('plugin_remove', { id: p.id }))}
            >
              {t('Remove')}
            </button>
          </div>
        </div>
      ))}
    </section>
  );
}
