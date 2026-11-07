import { useState } from 'react';
import { saveSettings, useAppSettings } from '../lib/settings';
import type { Account, AppSettings, Provider } from '../lib/types';
import { NO_LIMITS, OrchestraLimitsSettings } from '../features/orchestra/scheduling';
import { Switch } from './controls';
import LocalAssistantSettings from '../features/local-assistant/LocalAssistantSettings';
import { localAssistant } from '../features/local-assistant/instance';
import { useI18n } from '../lib/i18n';

/** Settings → Teamwork: how the providers help each other. */
export default function TeamworkSettings({
  advanced,
  accounts,
  onError,
  simpleOnly = false,
}: {
  advanced: boolean;
  accounts: Account[];
  onError: (s: string) => void;
  simpleOnly?: boolean;
}) {
  const { t } = useI18n();
  const s = useAppSettings(onError);
  const [busy, setBusy] = useState(false);
  async function save(next: AppSettings) {
    setBusy(true);
    try {
      await saveSettings(next);
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  }
  if (!s) return null;
  const providers = [...new Set(accounts.map((a) => a.provider))].filter(
    (p): p is Provider => p !== 'ollama',
  );
  if (simpleOnly) {
    return (
      <section className="settings-section">
        <Switch
          label="Orchestra"
          checked={s.orchestra}
          disabled={busy}
          onChange={(v) => void save({ ...s, orchestra: v })}
        />
      </section>
    );
  }
  return (
    <section className="settings-section">
      <h3>{t('Teamwork')}</h3>
      <p>{t('How Claude, ChatGPT and the other connected assistants help each other.')}</p>
      <Switch
        label="Orchestra"
        hint={t('For complex tasks, plan and split work across agents. Works with Auto.')}
        checked={s.orchestra}
        disabled={busy}
        onChange={(v) => void save({ ...s, orchestra: v })}
      />
      {s.orchestra && (
        <OrchestraLimitsSettings
          value={s.orchestraLimits ?? NO_LIMITS}
          providers={providers}
          disabled={busy}
          onChange={(orchestraLimits) => void save({ ...s, orchestraLimits })}
        />
      )}
      <Switch
        label={t('Switch on rate limit')}
        hint={t('If one provider hits its limit, another continues the same message.')}
        checked={s.autoFallback}
        disabled={busy}
        onChange={(v) => void save({ ...s, autoFallback: v })}
      />
      <LocalAssistantSettings
        assistant={localAssistant}
        config={s.localAssistant}
        onChange={(next) => void save({ ...s, localAssistant: next })}
      />
      <Switch
        label={t('Repository map references')}
        hint={t(
          'When a request names a class, function, scene or table, add where it is declared to the prompt (at most 12 locations, about 600 tokens). The request shows what was added.',
        )}
        checked={s.repoContext}
        disabled={busy}
        onChange={(v) => void save({ ...s, repoContext: v })}
      />
      {advanced && (
        <>
          <Switch
            label={t('Self-healing')}
            hint={t(
              'After edits, run the project check and fix failures automatically (up to 3 times).',
            )}
            checked={s.selfHeal}
            disabled={busy}
            onChange={(v) => void save({ ...s, selfHeal: v })}
          />
          <div className="switch-row">
            <span>
              <strong>{t('Cross-audit rounds')}</strong>
              <small>{t('How many review and revise cycles an audit may run.')}</small>
            </span>
            <div className="auto-policy" role="group" aria-label={t('Cross-audit rounds')}>
              {[1, 2, 3].map((n) => (
                <button
                  key={n}
                  aria-pressed={s.reviewRounds === n}
                  disabled={busy}
                  onClick={() => void save({ ...s, reviewRounds: n })}
                >
                  {n}
                </button>
              ))}
            </div>
          </div>
        </>
      )}
    </section>
  );
}
