import { useEffect, useRef, useState } from 'react';
import { Check, Pencil, RefreshCw, Settings2, X } from 'lucide-react';
import { call } from '../lib/bridge';
import type { Account, ModelInfo, Provider } from '../lib/types';
import { errorText } from '../lib/util';

export function AccountRename({
  account,
  done,
  onError,
}: {
  account: Account;
  done: () => Promise<void> | void;
  onError: (text: string) => void;
}) {
  const [name, setName] = useState(account.name);
  const [saving, setSaving] = useState(false);
  return (
    <form
      className="account-rename"
      onSubmit={(e) => {
        e.preventDefault();
        setSaving(true);
        void call('rename_account', { id: account.id, name: name.trim() })
          .then(done)
          .catch((e) => onError(errorText(e)))
          .finally(() => setSaving(false));
      }}
    >
      <input
        autoFocus
        aria-label="Account name"
        value={name}
        maxLength={80}
        onChange={(e) => setName(e.target.value)}
        required
      />
      <button
        className="icon-button"
        aria-label="Save account name"
        disabled={saving || !name.trim()}
        type="submit"
      >
        <Check size={15} />
      </button>
      <button
        className="icon-button"
        aria-label="Cancel account rename"
        type="button"
        onClick={() => void done()}
      >
        <X size={15} />
      </button>
    </form>
  );
}

export default function ProviderOptions({
  provider,
  visible,
  accounts,
  accountId,
  setAccountId,
  model,
  setModel,
  effort,
  setEffort,
  manage,
  refreshAccounts,
  onError,
  onReady,
  paused,
}: {
  provider: Provider;
  visible: boolean;
  accounts: Account[];
  accountId: string;
  setAccountId: (id: string) => void;
  model: string;
  setModel: (id: string) => void;
  effort: string;
  setEffort: (level: string) => void;
  manage: () => void;
  refreshAccounts: () => Promise<void>;
  onError: (text: string) => void;
  onReady: (ready: boolean) => void;
  paused: boolean;
}) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  const [rename, setRename] = useState(false);
  const matching = accounts.filter((a) => a.provider === provider);
  const account = matching.find((a) => a.id === accountId) || matching[0];
  const effectiveId = account?.id;
  const lastAccount = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (paused) return;
    const keepSelection = lastAccount.current === effectiveId;
    const previousModel = model,
      previousEffort = effort;
    lastAccount.current = effectiveId;
    let gone = false;
    setModels([]);
    setError('');
    setRename(false);
    setModel('');
    setEffort('');
    onReady(false);
    if (!effectiveId) {
      setAccountId('');
      setLoading(false);
      return;
    }
    setAccountId(effectiveId);
    setLoading(true);
    void call<ModelInfo[]>('list_models', { id: effectiveId, refresh: revision > 0 })
      .then((rows) => {
        if (gone) return;
        setModels(rows);
        const preferred =
          (keepSelection && rows.find((m) => m.id === previousModel)) ||
          rows.find((m) => m.isDefault) ||
          rows[0];
        setModel(preferred?.id || '');
        setEffort(
          keepSelection && preferred?.efforts.includes(previousEffort)
            ? previousEffort
            : preferred?.defaultEffort || '',
        );
        onReady(rows.length > 0);
      })
      .catch((e) => {
        if (!gone) setError(errorText(e));
      })
      .finally(() => {
        if (!gone) setLoading(false);
      });
    return () => {
      gone = true;
    };
    // Re-query on identity/refresh/dialog changes; changing a model must not restart discovery.
  }, [effectiveId, provider, revision, setModel, setEffort, onReady, paused]);
  const selected = models.find((m) => m.id === model);
  return (
    <div hidden={!visible}>
      <div className="advanced-controls">
        <div>
          <div className="field-heading">
            <label htmlFor="account-picker">Account</label>
            <div className="field-actions">
              <button
                type="button"
                className="icon-button"
                disabled={!account}
                aria-label="Rename selected account"
                onClick={() => setRename(!rename)}
              >
                <Pencil size={13} />
              </button>
              <button
                type="button"
                className="icon-button"
                aria-label="Manage accounts"
                onClick={manage}
              >
                <Settings2 size={14} />
              </button>
            </div>
          </div>
          <select
            id="account-picker"
            aria-label="Account"
            value={effectiveId || ''}
            onChange={(e) => setAccountId(e.target.value)}
            disabled={!matching.length}
          >
            {!matching.length && <option value="">Add an account</option>}
            {matching.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
        </div>
        <div>
          <div className="field-heading">
            <label htmlFor="model-picker">Model</label>
            <button
              type="button"
              className="icon-button"
              disabled={!account || loading}
              aria-label="Refresh official models"
              onClick={() => setRevision((v) => v + 1)}
            >
              <RefreshCw size={13} className={loading ? 'spin' : ''} />
            </button>
          </div>
          <select
            id="model-picker"
            aria-label="Model"
            disabled={loading || !models.length}
            value={model}
            onChange={(e) => {
              const next = models.find((m) => m.id === e.target.value);
              setModel(e.target.value);
              setEffort(next?.defaultEffort || '');
            }}
          >
            {!models.length && (
              <option value="">
                {loading
                  ? 'Loading models…'
                  : account
                    ? 'Models unavailable'
                    : 'Connect an account'}
              </option>
            )}
            {models.map((m) => (
              <option key={m.id} value={m.id} title={m.description}>
                {m.name}
              </option>
            ))}
          </select>
        </div>
        <label className="reasoning-field">
          Reasoning
          <select
            aria-label="Reasoning effort"
            value={effort}
            disabled={loading || !selected?.efforts.length}
            onChange={(e) => setEffort(e.target.value)}
          >
            <option value="">Provider default</option>
            {selected?.efforts.map((level) => (
              <option key={level} value={level}>
                {level}
              </option>
            ))}
          </select>
        </label>
      </div>
      {rename && account && (
        <AccountRename
          account={account}
          onError={onError}
          done={async () => {
            await refreshAccounts();
            setRename(false);
          }}
        />
      )}
      {error && (
        <div className="catalog-status" role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
