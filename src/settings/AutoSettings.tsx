import { useEffect, useState } from 'react';
import { call, desktop } from '../lib/bridge';
import type { Account } from '../lib/types';
import { useI18n } from '../lib/i18n';
import { Switch } from './controls';
interface Usage {
  accountId: string;
  windows: { usedPercent: number; resetsAt: number; windowMinutes: number }[];
  updatedAt: number;
  source: string;
}
const dateValue = (timestamp: number) => {
  const date = new Date(timestamp);
  return new Date(timestamp - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
};
const POLICIES = ['budget', 'default', 'quality'];

/** Settings → Auto: the routing policy and, in Advanced mode, usage balancing. */
export function AutoPreferences({
  advanced,
  autoPolicy,
  setAutoPolicy,
  balanceUsage,
  setBalanceUsage,
}: {
  advanced: boolean;
  autoPolicy: string;
  setAutoPolicy: (p: string) => void;
  balanceUsage: boolean;
  setBalanceUsage: (v: boolean) => void;
}) {
  const { t } = useI18n();
  return (
    <section className="settings-section">
      <h3>{t('Auto preferences')}</h3>
      <p>{t('Choose how Auto weighs speed, quality and the remaining provider quota.')}</p>
      <div className="auto-controls settings-auto-controls">
        <div className="auto-policy" role="group" aria-label={t('Auto policy')}>
          {POLICIES.map((p) => (
            <button
              key={p}
              aria-pressed={autoPolicy === p}
              onClick={() => {
                setAutoPolicy(p);
                localStorage.setItem('lumen-auto-policy', p);
              }}
            >
              {t(p[0].toUpperCase() + p.slice(1))}
            </button>
          ))}
        </div>
      </div>
      {advanced && (
        <Switch
          label={t('Balance usage')}
          hint={t('Prefer accounts with more quota left before their reset.')}
          checked={balanceUsage}
          onChange={(v) => {
            setBalanceUsage(v);
            localStorage.setItem('lumen-balance-usage', String(v));
          }}
        />
      )}
    </section>
  );
}

export default function AutoSettings({
  accounts,
  onError,
}: {
  accounts: Account[];
  onError: (s: string) => void;
}) {
  const { t } = useI18n();
  const [usage, setUsage] = useState<Usage[]>([]);
  const [id, setId] = useState(accounts[0]?.id || '');
  const [busy, setBusy] = useState(false);
  const [percent, setPercent] = useState(['', '']);
  const [reset, setReset] = useState(['', '']);
  async function load() {
    setUsage(await call<Usage[]>('get_usage'));
  }
  useEffect(() => {
    if (desktop) void load().catch((e) => onError(String(e)));
  }, [onError]);
  useEffect(() => {
    const u = usage.find((u) => u.accountId === id);
    setPercent(
      [300, 10080].map((minutes) =>
        String(u?.windows.find((w) => w.windowMinutes === minutes)?.usedPercent ?? ''),
      ),
    );
    setReset(
      [300, 10080].map((minutes) => {
        const at = u?.windows.find((w) => w.windowMinutes === minutes)?.resetsAt;
        return at ? dateValue(at) : '';
      }),
    );
  }, [usage, id]);
  const selected = usage.find((u) => u.accountId === id);
  return (
    <section className="settings-section">
      <h3>{t('Auto usage balancing')}</h3>
      <p>
        {t(
          'Balance remaining quota against time until reset. Fresh snapshots last 15 minutes. Unknown usage never counts as free capacity.',
        )}
      </p>
      <div className="quota-editor">
        <label>
          {t('Account')}
          <select
            aria-label={t('Usage account')}
            value={id}
            onChange={(e) => setId(e.target.value)}
          >
            <option value="" disabled>
              {t('Choose account')}
            </option>
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
        </label>
        <button
          disabled={!desktop || !id || busy}
          className="folder-button"
          onClick={() => {
            setBusy(true);
            void call('refresh_usage', { id })
              .then(load)
              .catch((e) => onError(String(e)))
              .finally(() => setBusy(false));
          }}
        >
          {t('Refresh official quota')}
        </button>
        {selected && (
          <p>
            {selected.source} · {new Date(selected.updatedAt).toLocaleString()}{' '}
            {Date.now() - selected.updatedAt > 900000 ? `· ${t('Stale')}` : ''}
          </p>
        )}
        <p>
          {t(
            'Codex supports automatic quota queries. For Claude, copy the percentages and reset times from /usage. Leave a window blank if it is unavailable.',
          )}
        </p>
        {['5-hour', 'Weekly'].map((name, i) => (
          <div key={name} className="quota-window">
            <label>
              {t(name)} {t('used %')}
              <input
                aria-label={`${name} used percent`}
                type="number"
                min="0"
                max="100"
                step="0.1"
                value={percent[i]}
                onChange={(e) => setPercent((p) => p.map((x, j) => (i === j ? e.target.value : x)))}
              />
            </label>
            <label>
              {t('Reset (local time)')}
              <input
                aria-label={`${name} reset`}
                type="datetime-local"
                value={reset[i]}
                onChange={(e) => setReset((p) => p.map((x, j) => (i === j ? e.target.value : x)))}
              />
            </label>
          </div>
        ))}
        <button
          className="primary-button"
          disabled={!desktop || !id || busy}
          onClick={() => {
            const windows = [300, 10080].flatMap((windowMinutes, i) =>
              percent[i] !== '' && reset[i]
                ? [
                    {
                      windowMinutes,
                      usedPercent: Number(percent[i]),
                      resetsAt: new Date(reset[i]).getTime(),
                    },
                  ]
                : [],
            );
            if (!windows.length) {
              onError(t('Enter at least one quota percentage and reset time.'));
              return;
            }
            setBusy(true);
            void call('save_usage', {
              usage: { accountId: id, windows, updatedAt: Date.now(), source: 'manual' },
            })
              .then(load)
              .catch((e) => onError(String(e)))
              .finally(() => setBusy(false));
          }}
        >
          {t('Save quota snapshot')}
        </button>
      </div>
    </section>
  );
}
