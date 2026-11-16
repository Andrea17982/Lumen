import { useEffect, useState } from 'react';
import { call, external } from '../lib/bridge';
import type { Account } from '../lib/types';
import { errorText } from '../lib/util';
interface Config {
  model: string;
  autoEnabled: boolean;
  budgetUsd: number;
  inputPerMillion: number;
  outputPerMillion: number;
  maxOutputTokens: number;
  ratesConfirmed: boolean;
}
interface State {
  config: Config;
  keySaved: boolean;
  spentUsd: number;
}
export default function ApiAccountSettings({
  account,
  onError,
}: {
  account: Account;
  onError: (s: string) => void;
}) {
  const [state, setState] = useState<State | null>(null);
  const [secret, setSecret] = useState('');
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState('');
  const [loadError, setLoadError] = useState('');
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    setLoadError('');
    call<State>('get_api_settings', { id: account.id })
      .then((s) => {
        if (active)
          setState({
            ...s,
            config: { ...s.config, maxOutputTokens: s.config.maxOutputTokens || 4096 },
          });
      })
      .catch((e) => {
        if (active) setLoadError(errorText(e));
      });
    return () => {
      active = false;
    };
  }, [account.id, attempt]);
  const update = (value: Partial<Config>) =>
    setState((s) =>
      s
        ? {
            ...s,
            config: {
              ...s.config,
              ...value,
              ...('model' in value || 'inputPerMillion' in value || 'outputPerMillion' in value
                ? { ratesConfirmed: false }
                : {}),
            },
          }
        : s,
    );
  const action = async (run: () => Promise<void>) => {
    setBusy(true);
    setSaved('');
    try {
      await run();
    } catch (e) {
      onError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  const pricing =
    account.provider === 'openaiapi'
      ? 'https://openai.com/api/pricing/'
      : account.provider === 'anthropicapi'
        ? 'https://platform.claude.com/docs/en/about-claude/pricing'
        : 'https://ai.google.dev/gemini-api/docs/pricing';
  return (
    <details className="raw-config api-account-settings">
      <summary>API key and budget</summary>
      {loadError ? (
        <div role="alert">
          <p>{loadError}</p>
          <button className="text-button" onClick={() => setAttempt((v) => v + 1)}>
            Retry
          </button>
        </div>
      ) : !state ? (
        <p role="status">Loading API settings…</p>
      ) : (
        <>
          <form
            className="account-form api-key-form"
            onSubmit={(e) => {
              e.preventDefault();
              void action(async () => {
                await call('save_api_key', { id: account.id, secret });
                setSecret('');
                setState((s) => (s ? { ...s, keySaved: true } : s));
                setSaved('Key saved in the OS keychain.');
              });
            }}
          >
            <label>
              API key for {account.name}
              <input
                type="password"
                autoComplete="off"
                spellCheck={false}
                value={secret}
                onChange={(e) => setSecret(e.target.value)}
                placeholder={
                  state.keySaved ? 'Key saved · enter a replacement' : 'Paste your official API key'
                }
              />
            </label>
            <button className="text-button" disabled={busy || !secret.trim()} type="submit">
              Save key
            </button>
          </form>
          <form
            className="account-form api-settings-grid"
            onSubmit={(e) => {
              e.preventDefault();
              void action(async () => {
                await call('save_api_settings', { id: account.id, value: state.config });
                setSaved('API settings saved.');
              });
            }}
          >
            <label>
              Priced model ID
              <input
                required
                value={state.config.model}
                onChange={(e) => update({ model: e.target.value })}
                autoComplete="off"
                spellCheck={false}
              />
            </label>
            <label>
              Total budget (USD)
              <input
                required
                type="number"
                min="0.01"
                max="1000000"
                step="any"
                value={state.config.budgetUsd}
                onChange={(e) => update({ budgetUsd: Number(e.target.value) })}
              />
            </label>
            <label>
              Input USD / 1M tokens
              <input
                required
                type="number"
                min="0"
                step="any"
                value={state.config.inputPerMillion}
                onChange={(e) => update({ inputPerMillion: Number(e.target.value) })}
              />
            </label>
            <label>
              Output USD / 1M tokens
              <input
                required
                type="number"
                min="0"
                step="any"
                value={state.config.outputPerMillion}
                onChange={(e) => update({ outputPerMillion: Number(e.target.value) })}
              />
            </label>
            <label>
              Maximum output tokens
              <input
                required
                type="number"
                min="256"
                max="32768"
                step="1"
                value={state.config.maxOutputTokens}
                onChange={(e) => update({ maxOutputTokens: Number(e.target.value) })}
              />
            </label>
            <p>
              Used/reserved estimate: ${state.spentUsd.toFixed(6)}. Uncertain requests retain a
              conservative charge estimate.
            </p>
            <label className="api-setting-check">
              <input
                type="checkbox"
                checked={state.config.ratesConfirmed}
                onChange={(e) => update({ ratesConfirmed: e.target.checked })}
              />
              I checked the highest applicable official rates for this model.
            </label>
            <button
              className="text-button"
              type="button"
              onClick={() => void external(pricing).catch((e) => onError(errorText(e)))}
            >
              Official pricing
            </button>
            <label className="api-setting-check">
              <input
                type="checkbox"
                checked={state.config.autoEnabled}
                onChange={(e) => update({ autoEnabled: e.target.checked })}
              />
              Allow Auto to use this API within the budget
            </label>
            <button
              className="primary-button"
              type="submit"
              disabled={busy || !state.keySaved || !state.config.ratesConfirmed}
            >
              Save API settings
            </button>
          </form>
          <p className="catalog-status">
            Prices apply only to the model above. Update them before switching models. Keys stay in
            your OS keychain; subscription CLIs never receive them. This budget covers model token
            estimates; external tool fees are separate.
          </p>
          {saved && <p role="status">{saved}</p>}
        </>
      )}
    </details>
  );
}
