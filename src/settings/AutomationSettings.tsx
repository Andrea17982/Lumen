import { useEffect, useState } from 'react';
import { Check, Copy } from 'lucide-react';
import { call, desktop } from '../lib/bridge';
import type { ApiStatus, AppSettings } from '../lib/types';

function Switch({
  label,
  hint,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  hint: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label className="switch-row">
      <span>
        <strong>{label}</strong>
        <small>{hint}</small>
      </span>
      <input
        type="checkbox"
        role="switch"
        className="switch"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
    </label>
  );
}

function CopyField({ label, value }: { label: string; value: string }) {
  const [done, setDone] = useState(false);
  return (
    <div className="copy-field">
      <span>{label}</span>
      <code>{value}</code>
      <button
        className="icon-button"
        aria-label={`Copy ${label}`}
        onClick={() =>
          void navigator.clipboard.writeText(value).then(() => {
            setDone(true);
            setTimeout(() => setDone(false), 1500);
          })
        }
      >
        {done ? <Check size={13} /> : <Copy size={13} />}
      </button>
    </div>
  );
}

export default function AutomationSettings({ onError }: { onError: (s: string) => void }) {
  const [s, setS] = useState<AppSettings | null>(null);
  const [api, setApi] = useState<ApiStatus | null>(null);
  const [port, setPort] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    void call<AppSettings>('get_settings')
      .then((v) => {
        setS(v);
        setPort(String(v.apiPort));
      })
      .catch((e) => onError(String(e)));
  }, [onError]);
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
      const v = await call<AppSettings>('save_settings', { value: next });
      setS(v);
      setApi(await call<ApiStatus>('api_status'));
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  }
  if (!s) return null;
  return (
    <>
      <section className="settings-section">
        <h3>Teamwork</h3>
        <p>How Claude and ChatGPT help each other.</p>
        <Switch
          label="Switch on rate limit"
          hint="If one provider hits its limit, the other continues the same message."
          checked={s.autoFallback}
          disabled={!desktop || busy}
          onChange={(v) => void save({ ...s, autoFallback: v })}
        />
        <Switch
          label="Self-healing"
          hint="After edits, run the project check and fix failures automatically (up to 3 times)."
          checked={s.selfHeal}
          disabled={!desktop || busy}
          onChange={(v) => void save({ ...s, selfHeal: v })}
        />
        <div className="switch-row">
          <span>
            <strong>Cross-audit rounds</strong>
            <small>How many review and revise cycles an audit may run.</small>
          </span>
          <div className="auto-policy" role="group" aria-label="Cross-audit rounds">
            {[1, 2, 3].map((n) => (
              <button
                key={n}
                aria-pressed={s.reviewRounds === n}
                disabled={!desktop || busy}
                onClick={() => void save({ ...s, reviewRounds: n })}
              >
                {n}
              </button>
            ))}
          </div>
        </div>
      </section>
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
    </>
  );
}
