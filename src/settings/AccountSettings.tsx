import { useEffect, useState } from 'react';
import { Circle, Pencil, Plus, ShieldCheck, Trash2 } from 'lucide-react';
import { AccountRename } from '../components/ProviderOptions';
import { call, external } from '../lib/bridge';
import { saveSettings, useAppSettings } from '../lib/settings';
import type { Account, Diagnostics, OllamaStatus, Provider } from '../lib/types';
import { errorText, nameOf, REPO } from '../lib/util';
import { useI18n } from '../lib/i18n';

export default function AccountSettings({
  accounts,
  refresh,
  onError,
  initialProvider,
  advanced = false,
}: {
  advanced?: boolean;
  accounts: Account[];
  initialProvider: Provider;
  refresh: () => Promise<void>;
  onError: (text: string) => void;
}) {
  const { t } = useI18n();
  const [name, setName] = useState('');
  const [provider, setProvider] = useState<Provider>(initialProvider);
  const [editing, setEditing] = useState('');
  const [statuses, setStatuses] = useState<Record<string, string>>({});
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const [adding, setAdding] = useState(false);
  const [pending, setPending] = useState(false);
  const [confirm, setConfirm] = useState('');
  const [ollama, setOllama] = useState<OllamaStatus | null>(null);
  const settings = useAppSettings(onError);
  const hasOllama = accounts.some((a) => a.provider === 'ollama');
  const checkOllama = () =>
    call<OllamaStatus>('ollama_status')
      .then(setOllama)
      .catch((e) => onError(errorText(e)));
  useEffect(() => {
    void call<Diagnostics>('diagnostics')
      .then(setDiag)
      .catch((e) => onError(errorText(e)));
  }, [onError]);
  useEffect(() => {
    if (hasOllama) void checkOllama();
  }, [hasOllama]); // eslint-disable-line react-hooks/exhaustive-deps
  const ollamaText = !ollama
    ? 'Checking…'
    : !ollama.installed
      ? 'Not installed'
      : !ollama.running
        ? 'Not running · open the Ollama app'
        : !ollama.models.length
          ? 'No models · run ollama pull llama3.2'
          : `${ollama.models.length} model${ollama.models.length === 1 ? '' : 's'} · ${ollama.selected}`;
  const ollamaState = !ollama?.installed
    ? ''
    : ollama.running && ollama.models.length
      ? 'active'
      : 'near';
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
          {t('Connected accounts')}{' '}
          <button
            className="help-button"
            aria-label={t('Account setup and provider requirements')}
            title={t('Account setup and provider requirements')}
            onClick={() =>
              void external(`${REPO}/blob/main/docs/accounts.md`).catch((e) =>
                onError(errorText(e)),
              )
            }
          >
            ?
          </button>
        </h3>
        <button
          className="icon-button"
          aria-label={t('Add account')}
          onClick={() => setAdding(!adding)}
        >
          <Plus size={17} />
        </button>
      </div>
      <p>
        {t(
          'Sign in through the official CLI. Each account gets its own configuration folder. Antigravity, Gemini CLI and Ollama (local) are emergency fallbacks in Auto, used only after Claude and GPT are rate limited or out of quota.',
        )}
      </p>
      {diag && (
        <div className="cli-status">
          <span className={diag.claude ? 'available' : ''}>
            <Circle size={7} fill="currentColor" />
            Claude CLI {t(diag.claude ? 'found' : 'not found')}
          </span>
          <span className={diag.gpt ? 'available' : ''}>
            <Circle size={7} fill="currentColor" />
            Codex CLI {t(diag.gpt ? 'found' : 'not found')}
          </span>
          <span className={diag.gemini ? 'available' : ''}>
            <Circle size={7} fill="currentColor" />
            Gemini CLI {t(diag.gemini ? 'found' : 'not found')}
          </span>
          <span className={diag.antigravity ? 'available' : ''}>
            <Circle size={7} fill="currentColor" />
            Antigravity CLI {t(diag.antigravity ? 'found' : 'not found')}
          </span>
          <span className={diag.ollama ? 'available' : ''}>
            <Circle size={7} fill="currentColor" />
            Ollama {t(diag.ollama ? 'found' : 'not found')}
          </span>
        </div>
      )}
      {accounts.length === 0 && (
        <div className="account-empty">
          {t('Connect an account to start your first conversation.')}
        </div>
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
                  : a.provider === 'ollama'
                    ? 'Ol'
                    : 'Ag'}
          </div>
          <div className="account-info">
            <strong>{a.name}</strong>
            <small>
              {a.provider === 'ollama' ? (
                <>
                  <span className={`health-dot ${ollamaState}`} aria-hidden="true" /> Ollama ·{' '}
                  {ollamaText}
                </>
              ) : (
                <>
                  {nameOf(a.provider)} · {t(statuses[a.id] || 'Connection unchecked')}
                </>
              )}
            </small>
          </div>
          {a.provider !== 'ollama' ? (
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
              {t('Sign in')}
            </button>
          ) : (
            ollama &&
            !ollama.installed && (
              <button
                className="text-button"
                onClick={() =>
                  void external('https://ollama.com/download').catch((e) => onError(errorText(e)))
                }
              >
                {t('Install Ollama')}
              </button>
            )
          )}
          <button
            className="icon-button"
            disabled={pending}
            aria-label={`Check ${a.name}`}
            onClick={() =>
              void action(async () => {
                if (a.provider === 'ollama') return checkOllama();
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
          {a.provider === 'ollama' && advanced && settings && ollama?.models.length ? (
            <label className="ollama-model">
              {t('Model')}
              <select
                aria-label={t('Ollama model')}
                value={settings.ollamaModel}
                disabled={pending}
                onChange={(e) =>
                  void action(async () => {
                    await saveSettings({ ...settings, ollamaModel: e.target.value });
                    await checkOllama();
                  })
                }
              >
                <option value="">{t('Automatic (largest)')}</option>
                {ollama.models.map((m) => (
                  <option key={m.name} value={m.name}>
                    {m.name} · {m.size}
                  </option>
                ))}
              </select>
            </label>
          ) : null}
          {confirm === a.id && (
            <div className="account-confirm">
              <span>{t('Remove this account and its local login files?')}</span>
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
                {t('Remove')}
              </button>
              <button className="text-button" onClick={() => setConfirm('')}>
                {t('Keep')}
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
            {t('Account name')}
            <input
              required
              maxLength={80}
              autoFocus
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('e.g. Personal')}
            />
          </label>
          <label>
            {t('Provider')}
            <select value={provider} onChange={(e) => setProvider(e.target.value as Provider)}>
              <option value="claude">Claude Code</option>
              <option value="gpt">ChatGPT / Codex</option>
              <option value="gemini">Gemini CLI (emergency)</option>
              <option value="antigravity">Antigravity CLI (emergency)</option>
              <option value="ollama">Ollama (local, emergency)</option>
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
