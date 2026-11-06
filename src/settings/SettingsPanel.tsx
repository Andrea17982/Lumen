import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import { AlertCircle, ArrowUpRight, Folder, Heart, Monitor, Moon, Sun, X } from 'lucide-react';
import { version } from '../../package.json';
import { external } from '../lib/bridge';
import type { Account, Provider } from '../lib/types';
import { errorText, Mark, PAYPAL, REPO } from '../lib/util';
import AccountSettings from './AccountSettings';
import AutoSettings from './AutoSettings';
import AutomationSettings from './AutomationSettings';
import IntegrationSettings from './IntegrationSettings';

export default function SettingsPanel({
  open,
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
          <div className="settings-header">
            <span />
            <button className="icon-button" aria-label="Close settings" onClick={onClose}>
              <X size={18} />
            </button>
          </div>
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
          <h2>Settings</h2>

          <AccountSettings
            accounts={accounts}
            refresh={refreshAccounts}
            onError={setError}
            initialProvider={initialProvider}
          />
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
            </div>
            <details className="raw-config">
              <summary>Claude quota (manual)</summary>
              <AutoSettings accounts={accounts} onError={setError} />
            </details>
          </section>
          <AutomationSettings onError={setError} />
          <IntegrationSettings onError={setError} />
          <section className="settings-section">
            <h3>Appearance</h3>
            <div className="theme-switch" role="group" aria-label="Theme">
              {[
                { id: 'light', Icon: Sun },
                { id: 'dark', Icon: Moon },
                { id: 'system', Icon: Monitor },
              ].map(({ id, Icon }) => (
                <button
                  key={id}
                  aria-pressed={theme === id}
                  className={theme === id ? 'active' : ''}
                  onClick={() => setTheme(id)}
                >
                  <Icon size={16} />
                  {id.charAt(0).toUpperCase() + id.slice(1)}
                </button>
              ))}
            </div>
          </section>
          <section className="settings-section">
            <h3>Conversation folder</h3>
            <p>Linked to this conversation. New conversations use your last selection.</p>
            <button
              className="folder-button"
              onClick={() => void chooseFolder().catch((e) => setError(errorText(e)))}
            >
              <Folder size={16} />
              <span>{workspace || 'Choose a folder'}</span>
              <ArrowUpRight size={14} />
            </button>
          </section>
          <footer className="settings-section about">
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

            <div className="version">
              <span>Lumen {version} · Open source · MIT</span>
              <button
                className="text-button"
                onClick={() => void external(`${REPO}/issues`).catch((e) => setError(errorText(e)))}
              >
                Share your feedback <ArrowUpRight size={12} />
              </button>
            </div>
          </footer>
        </motion.aside>
      )}
    </AnimatePresence>
  );
}
