import { useSettingsEditor } from '../lib/busy';
import type { Account, Provider } from '../lib/types';
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
  const { s, busy, save } = useSettingsEditor(onError);
  if (!s) return null;
  const providers = [...new Set(accounts.map((a) => a.provider))].filter(
    (p): p is Provider => p !== 'ollama',
  );
  if (simpleOnly) {
    return (
      <section className="settings-section">
        <h3>{t('Teamwork')}</h3>
        <p>{t('How Claude, ChatGPT and the other connected assistants help each other.')}</p>
        <Switch
          label="Orchestra"
          hint={t(
            'When enabled, split complex tasks across agents. Turn off for direct native sessions.',
          )}
          checked={s.orchestra}
          disabled={busy}
          onChange={(v) => save({ ...s, orchestra: v })}
        />
      </section>
    );
  }
  return (
    <section className="settings-section">
      <h3>{t('Teamwork')}</h3>
      <p>{t('How Claude, ChatGPT and the other connected assistants help each other.')}</p>
      <h4>Orchestra</h4>
      <Switch
        label="Orchestra"
        hint={t(
          'When enabled, split complex tasks across agents. Turn off for direct native sessions.',
        )}
        checked={s.orchestra}
        disabled={busy}
        onChange={(v) => save({ ...s, orchestra: v })}
      />
      {s.orchestra && (
        <OrchestraLimitsSettings
          value={s.orchestraLimits ?? NO_LIMITS}
          providers={providers}
          disabled={busy}
          onChange={(orchestraLimits) => save({ ...s, orchestraLimits })}
        />
      )}
      <h4>{t('Reliability')}</h4>
      <Switch
        label={t('Verify changes after native turns')}
        hint={t(
          'Run additional Lumen project checks after the CLI finishes. Native sessions otherwise use the CLI’s own tools and checks.',
        )}
        checked={!!s.verifyChanges}
        disabled={busy}
        onChange={(v) => save({ ...s, verifyChanges: v })}
      />
      <Switch
        label={t('Switch on rate limit')}
        hint={t('If one provider hits its limit, another continues the same message.')}
        checked={s.autoFallback}
        disabled={busy}
        onChange={(v) => save({ ...s, autoFallback: v })}
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
            onChange={(v) => save({ ...s, selfHeal: v })}
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
                  onClick={() => save({ ...s, reviewRounds: n })}
                >
                  {n}
                </button>
              ))}
            </div>
          </div>
        </>
      )}
      <h4>{t('Context')}</h4>
      <Switch
        label={t('Code graph')}
        hint={t(
          "Claude and Codex get Lumen's code graph as tools: definitions with exact line ranges, callers, file outlines, related files and the impact of a change. They read only the lines they need instead of searching whole files.",
        )}
        checked={s.repoContext}
        disabled={busy}
        onChange={(v) => save({ ...s, repoContext: v })}
      />
      <LocalAssistantSettings
        assistant={localAssistant}
        config={s.localAssistant}
        onChange={(next) => save({ ...s, localAssistant: next })}
      />
    </section>
  );
}
