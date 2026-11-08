import { useEffect, useState } from 'react';
import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import { AlertCircle, ArrowUpRight, Heart, X } from 'lucide-react';
import { version } from '../../package.json';
import { call, external } from '../lib/bridge';
import { advancedActive, useAppSettings } from '../lib/settings';
import type { Account, McpServer, Provider } from '../lib/types';
import { errorText, Mark, PAYPAL, REPO } from '../lib/util';
import AccountSettings from './AccountSettings';
import { AutoPreferences } from './AutoSettings';
import GeneralSettings from './GeneralSettings';
import IntegrationSettings from './IntegrationSettings';
import TeamworkSettings from './TeamworkSettings';
import { useI18n } from '../lib/i18n';

export type SettingsTab = 'general' | 'accounts' | 'auto' | 'teamwork' | 'tools' | 'about';
const TABS: { id: SettingsTab; label: string; advanced?: boolean }[] = [
  { id: 'general', label: 'General' },
  { id: 'accounts', label: 'Accounts' },
  { id: 'auto', label: 'Auto' },
  { id: 'teamwork', label: 'Teamwork' },
  { id: 'tools', label: 'Tools', advanced: true },
  { id: 'about', label: 'About' },
];
const MODE_KEY = 'lumen-settings-mode';

export default function SettingsPanel({
  open,
  tab: requestedTab,
  onClose,
  error,
  setError,
  accounts,
  refreshAccounts,
  initialProvider,
  autoPolicy,
  setAutoPolicy,
  balanceUsage,
  setBalanceUsage,
  theme,
  setTheme,
}: {
  open: boolean;
  tab?: SettingsTab;
  onClose: () => void;
  error: string;
  setError: (text: string) => void;
  accounts: Account[];
  refreshAccounts: () => Promise<void>;
  initialProvider: Provider;
  autoPolicy: string;
  setAutoPolicy: (p: string) => void;
  balanceUsage: boolean;
  setBalanceUsage: (v: boolean) => void;
  theme: string;
  setTheme: (t: string) => void;
}) {
  const { t } = useI18n();
  const reduced = useReducedMotion();
  const [advanced, setAdvanced] = useState(() => localStorage.getItem(MODE_KEY) === 'advanced');
  const [tab, setTab] = useState<SettingsTab>(requestedTab || 'general');
  const settings = useAppSettings(setError);
  const [mcpOn, setMcpOn] = useState(0);
  useEffect(() => {
    if (open) setTab(requestedTab || 'general');
  }, [open, requestedTab]);
  useEffect(() => {
    if (!open) return;
    const frame = requestAnimationFrame(() =>
      document.querySelector<HTMLElement>('.settings-panel button:not(:disabled)')?.focus(),
    );
    return () => cancelAnimationFrame(frame);
  }, [open]);
  const visible = TABS.filter((tabItem) => advanced || !tabItem.advanced);
  const current = visible.some((t) => t.id === tab) ? tab : 'general';
  // Enabled MCP servers live in the Advanced-only Tools tab, so Simple mode counts them.
  useEffect(() => {
    if (!open || advanced) return;
    void call<McpServer[]>('mcp_health')
      .then((list) => setMcpOn(list.filter((m) => m.enabled).length))
      .catch(() => setMcpOn(0));
  }, [open, advanced]);
  const hidden = advanced ? 0 : advancedActive(settings, balanceUsage, mcpOn);
  function mode(next: boolean) {
    setAdvanced(next);
    localStorage.setItem(MODE_KEY, next ? 'advanced' : 'simple');
  }
  return (
    <AnimatePresence>
      {open && (
        <motion.aside
          role="dialog"
          aria-modal="false"
          className="settings-panel"
          initial={reduced ? false : { x: 35, opacity: 0 }}
          animate={{ x: 0, opacity: 1 }}
          exit={
            reduced
              ? { x: 0, opacity: 1 }
              : {
                  x: 35,
                  opacity: 0,
                  transition: { duration: 0.14, ease: [0.4, 0, 1, 1] },
                }
          }
          transition={{ duration: reduced ? 0 : 0.24, ease: [0.16, 1, 0.3, 1] }}
          aria-label={t('Settings panel')}
        >
          <header className="settings-header">
            <div className="settings-heading">
              <button className="icon-button" aria-label={t('Close settings')} onClick={onClose}>
                <X size={18} />
              </button>
              <h2>{t('Settings')}</h2>
              <div
                className="auto-policy settings-mode"
                role="group"
                aria-label={t('Settings mode')}
              >
                <button aria-pressed={!advanced} onClick={() => mode(false)}>
                  {t('Simple')}
                </button>
                <button aria-pressed={advanced} onClick={() => mode(true)}>
                  {t('Advanced')}
                </button>
              </div>
            </div>
            <nav className="settings-tabs" role="tablist" aria-label={t('Settings sections')}>
              {visible.map((tabItem) => (
                <button
                  key={tabItem.id}
                  role="tab"
                  id={`settings-tab-${tabItem.id}`}
                  aria-selected={current === tabItem.id}
                  aria-controls="settings-body"
                  onClick={(e) => {
                    setTab(tabItem.id);
                    e.currentTarget.scrollIntoView({ block: 'nearest', inline: 'nearest' });
                  }}
                >
                  {t(tabItem.label)}
                </button>
              ))}
            </nav>
          </header>
          <div
            className="settings-body"
            id="settings-body"
            role="tabpanel"
            aria-labelledby={`settings-tab-${current}`}
          >
            {error && (
              <div role="alert" className="error-banner">
                <AlertCircle size={16} />
                <span>{error}</span>
                <button
                  className="icon-button"
                  aria-label={t('Dismiss settings error')}
                  onClick={() => setError('')}
                >
                  <X size={14} />
                </button>
              </div>
            )}
            {current === 'general' && (
              <GeneralSettings
                theme={theme}
                setTheme={setTheme}
                onError={setError}
                setAutoPolicy={setAutoPolicy}
                setBalanceUsage={setBalanceUsage}
              />
            )}
            {current === 'accounts' && (
              <AccountSettings
                accounts={accounts}
                refresh={refreshAccounts}
                onError={setError}
                initialProvider={initialProvider}
                advanced={advanced}
              />
            )}
            {current === 'auto' && (
              <>
                <AutoPreferences
                  advanced={advanced}
                  autoPolicy={autoPolicy}
                  setAutoPolicy={setAutoPolicy}
                  balanceUsage={balanceUsage}
                  setBalanceUsage={setBalanceUsage}
                />
              </>
            )}
            {current === 'teamwork' && (
              <TeamworkSettings
                advanced={advanced}
                simpleOnly={!advanced}
                accounts={accounts}
                onError={setError}
              />
            )}
            {current === 'tools' && <IntegrationSettings onError={setError} />}
            {current === 'about' && (
              <section className="settings-section about">
                <Mark small />
                <h3>{t('About Lumen')}</h3>
                <p>
                  {t(
                    'Lumen is completely AI Written, created at the direction of Andrea. Thank you to everyone who tries it and shares how it feels.',
                  )}
                </p>
                <button
                  className="donate-button"
                  onClick={() => void external(PAYPAL).catch((e) => setError(errorText(e)))}
                >
                  <Heart size={15} />
                  {t('Support Lumen on PayPal')}
                  <ArrowUpRight size={14} />
                </button>
                <small>{t('Optional donations · No features behind a paywall.')}</small>
              </section>
            )}
            {hidden > 0 && (
              <p className="hidden-advanced">
                {hidden} {t(hidden === 1 ? 'advanced option is on' : 'advanced options are on')} ·{' '}
                <button className="text-button" onClick={() => mode(true)}>
                  {t('Show')}
                </button>
              </p>
            )}
          </div>
          <footer className="settings-footer version">
            <span>
              Lumen {version} · {t('Open source')} · MIT
            </span>
            <button
              className="text-button"
              onClick={() => void external(`${REPO}/issues`).catch((e) => setError(errorText(e)))}
            >
              {t('Share your feedback')} <ArrowUpRight size={12} />
            </button>
          </footer>
        </motion.aside>
      )}
    </AnimatePresence>
  );
}
