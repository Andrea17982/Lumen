import { useEffect, useState } from 'react';
import { call, desktop } from '../lib/bridge';
import type { Account } from '../lib/types';
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
export default function AutoSettings({
  accounts,
  onError,
}: {
  accounts: Account[];
  onError: (s: string) => void;
}) {
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
      <h3>Auto usage balancing</h3>
      <p>
        Balance remaining quota against time until reset. Fresh snapshots last 15 minutes. Unknown
        usage never counts as free capacity.
      </p>
      <div className="quota-editor">
        <label>
          Account
          <select aria-label="Usage account" value={id} onChange={(e) => setId(e.target.value)}>
            <option value="" disabled>
              Choose account
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
          Refresh official quota
        </button>
        {selected && (
          <p>
            {selected.source} · {new Date(selected.updatedAt).toLocaleString()}{' '}
            {Date.now() - selected.updatedAt > 900000 ? '· Stale' : ''}
          </p>
        )}
        <p>
          Codex supports automatic quota queries. For Claude, copy the percentages and reset times
          from /usage. Leave a window blank if it is unavailable.
        </p>
        {['5-hour', 'Weekly'].map((name, i) => (
          <div key={name} className="quota-window">
            <label>
              {name} used %
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
              Reset (local time)
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
              onError('Enter at least one quota percentage and reset time.');
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
          Save quota snapshot
        </button>
      </div>
    </section>
  );
}
