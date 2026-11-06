import { useEffect, useState } from 'react';
import { Circle, Pencil, Plus, ShieldCheck, Trash2 } from 'lucide-react';
import { AccountRename } from '../components/ProviderOptions';
import { call, external } from '../lib/bridge';
import type { Account, Diagnostics, Provider } from '../lib/types';
import { errorText, nameOf, REPO } from '../lib/util';

export default function AccountSettings({
  accounts,
  refresh,
  onError,
  initialProvider,
}: {
  accounts: Account[];
  initialProvider: Provider;
  refresh: () => Promise<void>;
  onError: (text: string) => void;
}) {
  const [name, setName] = useState('');
  const [provider, setProvider] = useState<Provider>(initialProvider);
  const [editing, setEditing] = useState('');
  const [statuses, setStatuses] = useState<Record<string, string>>({});
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const [adding, setAdding] = useState(false);
  const [pending, setPending] = useState(false);
  const [confirm, setConfirm] = useState('');
  useEffect(() => {
    void call<Diagnostics>('diagnostics')
      .then(setDiag)
      .catch((e) => onError(errorText(e)));
  }, [onError]);
  async function action(fn: () => Promise<unknown>) {
    setPending(true);
    try {
      await fn();
    } catch (e) {
      onError(errorText(e));
    } finally {
      setPending(false);
    }
  }
  return (
    <section className="settings-section">
      <div className="section-title">
        <h3>
          Connected accounts{' '}
          <button
            className="help-button"
            aria-label="Account setup and provider requirements"
            title="Account setup and provider requirements"
            onClick={() =>
              void external(`${REPO}/blob/main/docs/accounts.md`).catch((e) =>
                onError(errorText(e)),
              )
            }
          >
            ?
          </button>
        </h3>
        <button className="icon-button" aria-label="Add account" onClick={() => setAdding(!adding)}>
          <Plus size={17} />
        </button>
      </div>
      <p>
        Sign in through the official CLI. Each account gets its own configuration folder. Gemini CLI
        and Antigravity are emergency fallbacks in Auto, used only after fresh quota data confirms
        Claude and GPT are exhausted.
      </p>
      {diag && (
        <div className="cli-status">
          <span className={diag.claude ? 'available' : ''}>
            <Circle size={7} fill="currentColor" />
            Claude CLI {diag.claude ? 'found' : 'not found'}
          </span>
          <span className={diag.gpt ? 'available' : ''}>
            <Circle size={7} fill="currentColor" />
            Codex CLI {diag.gpt ? 'found' : 'not found'}
          </span>
          <span className={diag.gemini ? 'available' : ''}>
            <Circle size={7} fill="currentColor" />
            Gemini CLI {diag.gemini ? 'found' : 'not found'}
          </span>
          <span className={diag.antigravity ? 'available' : ''}>
            <Circle size={7} fill="currentColor" />
            Antigravity CLI {diag.antigravity ? 'found' : 'not found'}
          </span>
        </div>
      )}
      {accounts.length === 0 && (
        <div className="account-empty">Connect an account to start your first conversation.</div>
      )}
      {accounts.map((a) => (
        <div className="account-row" key={a.id}>
          <div className={`provider-avatar ${a.provider}`}>
            {a.provider === 'claude'
              ? 'C'
              : a.provider === 'gpt'
                ? 'G'
                : a.provider === 'gemini'
                  ? 'Gm'
                  : 'Ag'}
          </div>
          <div className="account-info">
            <strong>{a.name}</strong>
            <small>
              {nameOf(a.provider)} · {statuses[a.id] || 'Connection unchecked'}
            </small>
          </div>
          <button
            className="text-button"
            disabled={pending}
            onClick={() =>
              void action(async () => {
                await call('login_account', { id: a.id });
                setStatuses((s) => ({ ...s, [a.id]: 'Finish login in terminal' }));
              })
            }
          >
            Sign in
          </button>
          <button
            className="icon-button"
            disabled={pending}
            aria-label={`Check ${a.name}`}
            onClick={() =>
              void action(async () => {
                const ready = await call<boolean>('account_status', { id: a.id });
                setStatuses((s) => ({ ...s, [a.id]: ready ? 'Connected' : 'Sign-in required' }));
              })
            }
          >
            <ShieldCheck size={15} />
          </button>
          <button
            className="icon-button"
            disabled={pending}
            aria-label={`Rename ${a.name}`}
            onClick={() => setEditing(editing === a.id ? '' : a.id)}
          >
            <Pencil size={14} />
          </button>
          {editing === a.id && (
            <AccountRename
              account={a}
              onError={onError}
              done={async () => {
                await refresh();
                setEditing('');
              }}
            />
          )}
          <button
            className="icon-button"
            disabled={pending}
            aria-label={`Remove ${a.name}`}
            onClick={() => setConfirm(a.id)}
          >
            <Trash2 size={14} />
          </button>
          {confirm === a.id && (
            <div className="account-confirm">
              <span>Remove this account and its local login files?</span>
              <button
                className="text-button danger"
                disabled={pending}
                onClick={() =>
                  void action(async () => {
                    await call('remove_account', { id: a.id });
                    await refresh();
                    setConfirm('');
                  })
                }
              >
                Remove
              </button>
              <button className="text-button" onClick={() => setConfirm('')}>
                Keep
              </button>
            </div>
          )}
        </div>
      ))}
      {adding && (
        <form
          className="account-form"
          onSubmit={(e) => {
            e.preventDefault();
            void action(async () => {
              await call('add_account', { name, provider });
              setName('');
              setAdding(false);
              await refresh();
            });
          }}
        >
          <label>
            Account name
            <input
              required
              maxLength={80}
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Personal"
            />
          </label>
          <label>
            Provider
            <select value={provider} onChange={(e) => setProvider(e.target.value as Provider)}>
              <option value="claude">Claude Code</option>
              <option value="gpt">ChatGPT / Codex</option>
              <option value="gemini">Gemini CLI (emergency)</option>
              <option value="antigravity">Antigravity CLI (emergency)</option>
            </select>
          </label>
          <button className="primary-button" disabled={pending || !name.trim()} type="submit">
            {pending ? 'Adding…' : 'Add account'}
          </button>
        </form>
      )}
    </section>
  );
}
