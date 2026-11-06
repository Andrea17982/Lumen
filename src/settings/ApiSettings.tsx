import { useEffect, useState } from 'react';
import { call, desktop } from '../lib/bridge';
import { saveSettings, useAppSettings } from '../lib/settings';
import type { ApiStatus, AppSettings } from '../lib/types';
import { CopyField, Switch } from './controls';

/** Settings → Tools → Local API (Advanced only). */
export default function ApiSettings({ onError }: { onError: (s: string) => void }) {
  const s = useAppSettings(onError);
  const [api, setApi] = useState<ApiStatus | null>(null);
  const [port, setPort] = useState('');
  const [busy, setBusy] = useState(false);
  const savedPort = s?.apiPort;
  useEffect(() => {
    if (savedPort) setPort(String(savedPort));
  }, [savedPort]);
  useEffect(() => {
    if (!s?.apiEnabled) return;
    const load = () =>
      void call<ApiStatus>('api_status')
        .then(setApi)
        .catch(() => {});
    load();
    const t = window.setInterval(load, 3000);
    return () => window.clearInterval(t);
  }, [s?.apiEnabled]);
  async function save(next: AppSettings) {
    setBusy(true);
    try {
      await saveSettings(next);
      setApi(await call<ApiStatus>('api_status'));
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  }
  if (!s) return null;
  return (
    <section className="settings-section">
      <h3>Local API</h3>
      <p>
        Let other apps on this PC use your subscriptions through an OpenAI-compatible endpoint.
        Read-only, local only.
      </p>
      <Switch
        label="Serve on localhost"
        hint="Models: lumen-auto, lumen-claude, lumen-chatgpt."
        checked={s.apiEnabled}
        disabled={!desktop || busy}
        onChange={(v) => void save({ ...s, apiEnabled: v, apiPort: Number(port) || 8317 })}
      />
      {s.apiEnabled && (
        <div className="api-box">
          <CopyField label="Base URL" value={api?.url || `http://localhost:${s.apiPort}/v1`} />
          <CopyField label="API key" value={s.apiToken} />
          <label className="port-field">
            Port
            <input
              inputMode="numeric"
              value={port}
              onChange={(e) => setPort(e.target.value.replace(/\D/g, '').slice(0, 5))}
              onBlur={() =>
                Number(port) !== s.apiPort && void save({ ...s, apiPort: Number(port) })
              }
            />
          </label>
          <div className="api-log" aria-label="Recent API requests">
            {api?.log.length ? (
              api.log.slice(0, 8).map((l) => (
                <div key={l.at + l.model}>
                  <span className={l.status === '200' ? 'ok' : 'bad'}>
                    {l.status === '200' ? '200' : 'Error'}
                  </span>
                  <span>{l.model}</span>
                  <small>
                    {new Date(l.at).toLocaleTimeString()} · {(l.millis / 1000).toFixed(1)}s
                    {l.stream ? ' · stream' : ''}
                  </small>
                </div>
              ))
            ) : (
              <small>No requests yet.</small>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
