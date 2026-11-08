import { useI18n } from '../lib/i18n';
import { Switch } from './controls';

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
