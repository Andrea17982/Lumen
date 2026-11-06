import { useEffect, useState } from 'react';
import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import { AlertCircle, ArrowUpRight, Heart, X } from 'lucide-react';
import { version } from '../../package.json';
import { external } from '../lib/bridge';
import { advancedActive, useAppSettings } from '../lib/settings';
import type { Account, Provider } from '../lib/types';
import { errorText, Mark, PAYPAL, REPO } from '../lib/util';
import AccountSettings from './AccountSettings';
import ApiSettings from './ApiSettings';
import AutoSettings from './AutoSettings';
import GeneralSettings from './GeneralSettings';
import IntegrationSettings from './IntegrationSettings';
import TeamworkSettings from './TeamworkSettings';

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
  workspace,
  chooseFolder,
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
  workspace: string;
  chooseFolder: () => Promise<unknown>;
}) {
  const reduced = useReducedMotion();
  const [advanced, setAdvanced] = useState(() => localStorage.getItem(MODE_KEY) === 'advanced');
  const [tab, setTab] = useState<SettingsTab>(requestedTab || 'general');
  const settings = useAppSettings(setError);
  useEffect(() => {
    if (open) setTab(requestedTab || 'general');
  }, [open, requestedTab]);
  const visible = TABS.filter((t) => advanced || !t.advanced);
  const current = visible.some((t) => t.id === tab) ? tab : 'general';
  const hidden = advanced ? 0 : advancedActive(settings, balanceUsage);
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
          exit={reduced ? { x: 0, opacity: 1 } : { x: 35, opacity: 0 }}
          transition={{ duration: reduced ? 0 : 0.2 }}
          aria-label="Settings panel"
        >
          <header className="settings-header">
            <div className="settings-heading">
              <button className="icon-button" aria-label="Close settings" onClick={onClose}>
                <X size={18} />
              </button>
              <h2>Settings</h2>
              <div className="auto-policy settings-mode" role="group" aria-label="Settings mode">
                <button aria-pressed={!advanced} onClick={() => mode(false)}>
                  Simple
                </button>
                <button aria-pressed={advanced} onClick={() => mode(true)}>
                  Advanced
                </button>
              </div>
            </div>
            <nav className="settings-tabs" role="tablist" aria-label="Settings sections">
              {visible.map((t) => (
                <button
                  key={t.id}
                  role="tab"
                  id={`settings-tab-${t.id}`}
                  aria-selected={current === t.id}
                  aria-controls="settings-body"
                  onClick={() => setTab(t.id)}
                >
                  {t.label}
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
                  aria-label="Dismiss settings error"
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
                workspace={workspace}
                chooseFolder={chooseFolder}
                onError={setError}
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
                <section className="settings-section">
                  <h3>Auto preferences</h3>
                  <p>Choose how Auto weighs speed, quality and the remaining provider quota.</p>
                  <div className="auto-controls settings-auto-controls">
                    <div className="auto-policy" role="group" aria-label="Auto policy">
                      {['budget', 'default', 'quality'].map((p) => (
                        <button
                          key={p}
                          aria-pressed={autoPolicy === p}
                          onClick={() => {
                            setAutoPolicy(p);
                            localStorage.setItem('lumen-auto-policy', p);
                          }}
                        >
                          {p[0].toUpperCase() + p.slice(1)}
                        </button>
                      ))}
                    </div>
                    {advanced && (
                      <label>
                        <input
                          type="checkbox"
                          checked={balanceUsage}
                          onChange={(e) => {
                            setBalanceUsage(e.target.checked);
                            localStorage.setItem('lumen-balance-usage', String(e.target.checked));
                          }}
                        />{' '}
                        Balance usage
                      </label>
                    )}
                  </div>
                </section>
                {advanced && <AutoSettings accounts={accounts} onError={setError} />}
              </>
            )}
            {current === 'teamwork' && <TeamworkSettings advanced={advanced} onError={setError} />}
            {current === 'tools' && (
              <>
                <IntegrationSettings onError={setError} />
                <ApiSettings onError={setError} />
              </>
            )}
            {current === 'about' && (
              <section className="settings-section about">
                <Mark small />
                <p>
                  Lumen is completely AI Written, created at the direction of Andrea. Thank you to
                  everyone who tries it and shares how it feels.
                </p>
                <button
                  className="donate-button"
                  onClick={() => void external(PAYPAL).catch((e) => setError(errorText(e)))}
                >
                  <Heart size={15} />
                  Support Lumen on PayPal
                  <ArrowUpRight size={14} />
                </button>
                <small>Optional donations · No features behind a paywall.</small>
              </section>
            )}
            {hidden > 0 && (
              <p className="hidden-advanced">
                {hidden} advanced {hidden === 1 ? 'option is' : 'options are'} on ·{' '}
                <button className="text-button" onClick={() => mode(true)}>
                  Show
                </button>
              </p>
            )}
          </div>
          <footer className="settings-footer version">
            <span>Lumen {version} · Open source · MIT</span>
            <button
              className="text-button"
              onClick={() => void external(`${REPO}/issues`).catch((e) => setError(errorText(e)))}
            >
              Share your feedback <ArrowUpRight size={12} />
            </button>
          </footer>
        </motion.aside>
      )}
    </AnimatePresence>
  );
}
