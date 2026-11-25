import { useEffect, useState } from 'react';
import { Badge, Button, ConfirmDialog, Menu, Select, Switch, TextField } from '../../design/components';
import { useLocale, useT } from '../../i18n';
import { formatPercent, formatRelative, formatTime, languageTag } from '../../i18n/format';
import type { Account, AccountUsage, Provider } from '../../protocol/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import s from './Settings.module.css';

const providerName: Record<Provider, string> = {
  claude: 'Claude Code',
  gpt: 'Codex',
  antigravity: 'Antigravity',
  opencode: 'OpenCode',
  cursor: 'Cursor',
  ollama: 'Ollama',
  anthropicapi: 'Anthropic API',
  openaiapi: 'OpenAI API',
  googleapi: 'Google API',
};
const providerOptions = (Object.keys(providerName) as Provider[]).map((id) => ({ id, label: providerName[id] }));
const cliName: Record<string, string> = {
  'claude-cli': 'Claude Code',
  'codex-cli': 'Codex',
  'antigravity-cli': 'Antigravity',
};
const isApi = (p: Provider) => p.endsWith('api');
const SHOWN_CAPABILITIES = ['nativeResume', 'askFirst', 'cancellation', 'inputImages', 'subagents', 'fallback'] as const;

interface Health {
  accountId: string;
  state: string;
  until: number | null;
}
type Capability = {
  support: 'available' | 'limited' | 'unavailable';
  note: string;
};
type Contract = { provider: Provider } & Record<string, Capability | string>;

/**
 * The agents Lumen may use. Each one signs in through its official app; quota comes from what
 * that app reports, and is shown as missing when it reported nothing, never as zero.
 */
export function Accounts() {
  const t = useT();
  const sync = useSyncClient();
  const accounts = useSyncState((st) => st.accounts);
  const usage = useSyncState((st) => st.usage);
  const [health, setHealth] = useState<Health[]>([]);
  const [contracts, setContracts] = useState<Contract[]>([]);
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    sync.call<Health[]>('provider_health').then(setHealth, () => {});
    sync.call<{ adapters: Contract[] }>('adapter_capabilities').then(
      (m) => setContracts(m.adapters),
      () => {},
    );
  }, [sync, accounts]);

  return (
    <div className={s.section}>
      <p className={s.intro}>{t.accounts.intro}</p>
      {accounts.length === 0 && <p className={s.quiet}>{t.accounts.none}</p>}
      <ul className={s.cards}>
        {accounts.map((a) => (
          <AccountCard key={a.id} account={a} usage={usage.find((u) => u.accountId === a.id)} health={health.find((h) => h.accountId === a.id)} contract={contracts.find((c) => c.provider === a.provider)} onError={setError} />
        ))}
      </ul>
      <div>
        <Button variant="secondary" icon="plus" onPress={() => setAdding(true)}>
          {t.accounts.add}
        </Button>
      </div>
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
      <AddAccount
        open={adding}
        onClose={() => setAdding(false)}
        onAdd={async (name, provider) => {
          setError(null);
          try {
            const account = await sync.call<Account>('add_account', {
              name,
              provider,
            });
            await sync.loadAccounts();
            // It just works: a CLI account goes straight to its official sign-in.
            if (!isApi(provider) && provider !== 'ollama') await sync.call('login_account', { id: account.id });
          } catch (e) {
            setError(String(e));
          }
        }}
      />
    </div>
  );
}

function AccountCard({ account, usage, health, contract, onError }: { account: Account; usage?: AccountUsage; health?: Health; contract?: Contract; onError: (e: string) => void }) {
  const t = useT();
  const locale = useLocale();
  const sync = useSyncClient();
  const [status, setStatus] = useState<boolean | null>(null);
  const [checking, setChecking] = useState(false);
  const [dialog, setDialog] = useState<'rename' | 'remove' | null>(null);
  const [name, setName] = useState(account.name);
  const api = isApi(account.provider);

  async function check() {
    setChecking(true);
    try {
      setStatus(await sync.call<boolean>('account_status', { id: account.id }));
      if (!api && account.provider !== 'ollama') await sync.call('refresh_usage', { id: account.id }).catch(() => {});
      await sync.loadAccounts();
    } catch (e) {
      onError(String(e));
    } finally {
      setChecking(false);
    }
  }
  useEffect(() => {
    sync.call<boolean>('account_status', { id: account.id }).then(setStatus, () => setStatus(false));
  }, [sync, account.id]);

  const resting = health?.state === 'cooldown' && health.until ? health.until : null;
  return (
    <li className={s.card} aria-label={`${account.name}, ${providerName[account.provider]}`}>
      <div className={s.cardHead}>
        <div className={s.cardTitle}>
          <strong>{account.name}</strong>
          <span>{providerName[account.provider]}</span>
        </div>
        {status === null || checking ? <Badge>{t.accounts.checking}</Badge> : status ? <Badge tone="success">{t.accounts.connected}</Badge> : <Badge tone="warning">{t.accounts.notConnected}</Badge>}
        <Menu
          label={t.moreActions}
          items={[
            { id: 'rename', label: t.accounts.rename },
            {
              id: 'remove',
              label: t.accounts.remove,
              danger: true,
              separatorBefore: true,
            },
          ]}
          onAction={(id) => setDialog(id as 'rename' | 'remove')}
        />
      </div>
      {resting && <p className={s.quiet}>{t.accounts.resting(formatTime(locale, resting))}</p>}
      {!api && account.provider !== 'ollama' && <Quota usage={usage} />}
      {account.provider === 'ollama' && <OllamaState />}
      {api && <ApiSettings account={account} onError={onError} onSaved={check} />}
      <div className={s.actions}>
        {!api && account.provider !== 'ollama' && !status && (
          <Button variant="primary" onPress={() => sync.call('login_account', { id: account.id }).catch((e) => onError(String(e)))}>
            {t.accounts.signIn}
          </Button>
        )}
        <Button variant="secondary" isDisabled={checking} onPress={() => void check()}>
          {t.accounts.check}
        </Button>
      </div>
      {!api && account.provider !== 'ollama' && !status && <p className={s.help}>{t.accounts.signInHelp}</p>}
      {contract && (
        <details className={s.capabilities}>
          <summary>{t.accounts.can}</summary>
          <dl>
            {SHOWN_CAPABILITIES.map((key) => {
              const c = contract[key] as Capability | undefined;
              return c ? (
                <div key={key}>
                  <dt>{t.accounts.capabilities[key]}</dt>
                  <dd title={c.note || undefined}>{t.accounts.support[c.support]}</dd>
                </div>
              ) : null;
            })}
          </dl>
        </details>
      )}
      <ConfirmDialog
        isOpen={dialog === 'rename'}
        onOpenChange={(o) => !o && setDialog(null)}
        title={t.accounts.rename}
        body={<TextField label={t.accounts.name} value={name} onChange={setName} />}
        confirmLabel={t.accounts.save}
        cancelLabel={t.cancel}
        onConfirm={() =>
          sync.call('rename_account', { id: account.id, name: name.trim() }).then(
            () => sync.loadAccounts(),
            (e) => onError(String(e)),
          )
        }
      />
      <ConfirmDialog
        isOpen={dialog === 'remove'}
        onOpenChange={(o) => !o && setDialog(null)}
        title={t.accounts.removeTitle(account.name)}
        body={t.accounts.removeBody}
        confirmLabel={t.accounts.remove}
        cancelLabel={t.cancel}
        danger
        onConfirm={() =>
          sync.call('remove_account', { id: account.id }).then(
            () => sync.loadAccounts(),
            (e) => onError(String(e)),
          )
        }
      />
    </li>
  );
}

/** One bar per window the CLI reported. Nothing reported shows as such, not as an empty bar. */
function Quota({ usage }: { usage?: AccountUsage }) {
  const t = useT();
  const locale = useLocale();
  if (!usage || usage.windows.length === 0) return <p className={s.quiet}>{t.accounts.notReported}</p>;
  return (
    <div className={s.quota}>
      {usage.windows.map((w, i) => {
        const label = t.accounts.windows[w.windowMinutes] ?? t.accounts.minutes(w.windowMinutes);
        const used = Math.max(0, Math.min(100, w.usedPercent));
        return (
          <div key={i} className={s.window}>
            <div className={s.windowHead}>
              <span>{w.group ? `${w.group} · ${label}` : label}</span>
              <span>
                {t.accounts.used(formatPercent(locale, used))} · {t.accounts.resets(formatRelative(locale, w.resetsAt))}
              </span>
            </div>
            <div className={s.bar} role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(used)} data-high={used >= 90 || undefined}>
              <span style={{ width: `${used}%` }} />
            </div>
          </div>
        );
      })}
      <p className={s.help}>{t.accounts.reported(cliName[usage.source] ?? usage.source)}</p>
    </div>
  );
}

function OllamaState() {
  const t = useT();
  const sync = useSyncClient();
  const [state, setState] = useState<{
    installed: boolean;
    running: boolean;
    models: unknown[];
  } | null>(null);
  useEffect(() => {
    sync.call<{ installed: boolean; running: boolean; models: unknown[] }>('ollama_status').then(setState, () => {});
  }, [sync]);
  if (!state) return null;
  return <p className={s.quiet}>{!state.installed ? t.accounts.ollamaMissing : !state.running ? t.accounts.ollamaStopped : t.accounts.ollamaModels(state.models.length)}</p>;
}

interface ApiConfig {
  model: string;
  autoEnabled: boolean;
  budgetUsd: number;
  inputPerMillion: number;
  outputPerMillion: number;
  maxOutputTokens: number;
  ratesConfirmed: boolean;
}
interface ApiState {
  config: ApiConfig;
  keySaved: boolean;
  spentUsd: number;
}
const amount = (v: string) => Number(v.replace(',', '.'));
const field = (n: number) => (n ? String(n) : '');

/**
 * The key is write-only: it goes to the keychain and never comes back to the page. Lumen reserves
 * the cost of each request before sending it, so it needs the model's rates and a budget first.
 */
function ApiSettings({ account, onError, onSaved }: { account: Account; onError: (e: string) => void; onSaved: () => void }) {
  const t = useT();
  const locale = useLocale();
  const sync = useSyncClient();
  const [state, setState] = useState<ApiState | null>(null);
  const [key, setKey] = useState('');
  const [form, setForm] = useState({
    model: '',
    input: '',
    output: '',
    budget: '',
    cap: '8192',
    confirmed: false,
  });
  const [saved, setSaved] = useState(false);
  const load = () =>
    sync.call<ApiState>('get_api_settings', { id: account.id }).then(
      (st) => {
        setState(st);
        const c = st.config;
        setForm({
          model: c.model,
          input: field(c.inputPerMillion),
          output: field(c.outputPerMillion),
          budget: field(c.budgetUsd),
          cap: String(c.maxOutputTokens || 8192),
          confirmed: c.ratesConfirmed,
        });
      },
      (e) => onError(String(e)),
    );
  useEffect(() => {
    void load();
  }, [account.id]);
  if (!state) return null;
  const save = (config: Partial<ApiConfig>) =>
    sync
      .call('save_api_settings', {
        id: account.id,
        value: { ...state.config, ...config },
      })
      .then(
        () => load().then(() => true),
        (e) => {
          onError(String(e));
          return false;
        },
      );
  const money = new Intl.NumberFormat(languageTag(locale), {
    style: 'currency',
    currency: 'USD',
  });
  const set = (k: keyof typeof form) => (v: string | boolean) => {
    setSaved(false);
    setForm({ ...form, [k]: v });
  };
  const ready = state.keySaved && state.config.ratesConfirmed && state.config.budgetUsd > 0;
  return (
    <div className={s.api}>
      <form
        className={s.row}
        onSubmit={(e) => {
          e.preventDefault();
          const secret = key;
          setKey('');
          sync.call('save_api_key', { id: account.id, secret }).then(
            () => {
              void load();
              onSaved();
            },
            (err) => onError(String(err)),
          );
        }}
      >
        <TextField label={t.accounts.key} type="password" autoComplete="off" value={key} onChange={setKey} description={state.keySaved ? t.accounts.keySaved : t.accounts.keyHelp} />
        <Button variant="secondary" type="submit" isDisabled={!key.trim()}>
          {t.accounts.saveKey}
        </Button>
      </form>
      <form
        className={s.api}
        onSubmit={(e) => {
          e.preventDefault();
          void save({
            model: form.model.trim(),
            inputPerMillion: amount(form.input),
            outputPerMillion: amount(form.output),
            budgetUsd: amount(form.budget),
            maxOutputTokens: Math.round(amount(form.cap)),
            ratesConfirmed: form.confirmed,
          }).then((ok) => setSaved(ok));
        }}
      >
        <TextField label={t.accounts.model} value={form.model} onChange={set('model')} description={t.accounts.modelHelp} />
        <div className={s.grid}>
          <TextField label={t.accounts.inputRate} value={form.input} onChange={set('input')} />
          <TextField label={t.accounts.outputRate} value={form.output} onChange={set('output')} />
          <TextField label={t.accounts.budget} value={form.budget} onChange={set('budget')} description={t.accounts.spent(money.format(state.spentUsd))} />
          <TextField label={t.accounts.outputCap} value={form.cap} onChange={set('cap')} />
        </div>
        <Switch isSelected={form.confirmed} onChange={set('confirmed')}>
          {t.accounts.ratesConfirmed}
        </Switch>
        <div className={s.actions}>
          <Button variant="secondary" type="submit" isDisabled={!form.model.trim() || !form.confirmed}>
            {t.accounts.saveModel}
          </Button>
          {saved && (
            <span className={s.quiet} role="status">
              {t.accounts.saved}
            </span>
          )}
        </div>
      </form>
      <Switch isSelected={state.config.autoEnabled} isDisabled={!ready} onChange={(autoEnabled) => void save({ autoEnabled })}>
        <span className={s.switchText}>
          {t.accounts.autoUse}
          <small>{t.accounts.autoUseHelp}</small>
        </span>
      </Switch>
    </div>
  );
}

function AddAccount({ open, onClose, onAdd }: { open: boolean; onClose: () => void; onAdd: (name: string, provider: Provider) => Promise<void> }) {
  const t = useT();
  const [provider, setProvider] = useState<Provider>('claude');
  const [name, setName] = useState(providerName.claude);
  useEffect(() => {
    if (open) {
      setProvider('claude');
      setName(providerName.claude);
    }
  }, [open]);
  return (
    <ConfirmDialog
      isOpen={open}
      onOpenChange={(o) => !o && onClose()}
      title={t.accounts.addTitle}
      body={
        <div className={s.dialogFields}>
          <Select
            label={t.accounts.provider}
            value={provider}
            options={providerOptions}
            onChange={(p) => {
              // The name follows the agent until the user types one.
              if (name === providerName[provider]) setName(providerName[p]);
              setProvider(p);
            }}
          />
          <TextField label={t.accounts.name} value={name} onChange={setName} />
        </div>
      }
      confirmLabel={t.accounts.add}
      cancelLabel={t.cancel}
      onConfirm={() => void onAdd(name.trim() || providerName[provider], provider)}
    />
  );
}
