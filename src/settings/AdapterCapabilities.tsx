import { useEffect, useState } from 'react';
import { call, desktop } from '../lib/bridge';
import { useI18n } from '../lib/i18n';
import type { Provider } from '../lib/types';
import { errorText } from '../lib/util';

type Support = 'available' | 'limited' | 'unavailable';
interface Capability {
  support: Support;
  note: string;
}
const FEATURES = {
  nativeResume: 'Native session resume',
  askFirst: 'Per-action approvals',
  nativeSkills: 'Native skills',
  sharedTools: 'Shared tools',
  inputUsage: 'Reported input tokens',
  turnCost: 'Cost per turn',
  cancellation: 'Stop a running turn',
  permissionProfiles: 'Permission profiles',
  streaming: 'Streaming output',
  inputImages: 'Native image input',
  subagents: 'Native subagents',
  nativeExtensions: 'Commands, hooks and extensions',
  modelChange: 'Model selection',
  fallback: 'Auto and fallback',
  events: 'Normalized events',
} as const;
type Feature = keyof typeof FEATURES;
type Adapter = { provider: Provider; label: string } & Record<Feature, Capability>;
interface Matrix {
  schemaVersion: number;
  adapters: Adapter[];
}
const STATUS: Record<Support, string> = {
  available: 'Supported',
  limited: 'Limited',
  unavailable: 'Unavailable',
};

/** Loaded on demand from the same backend contract used by execution. */
export default function AdapterCapabilities() {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [matrix, setMatrix] = useState<Matrix | null>(null);
  const [error, setError] = useState('');
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    if (!open || !desktop || matrix) return;
    let active = true;
    setError('');
    void call<Matrix>('adapter_capabilities')
      .then((value) => {
        if (value.schemaVersion !== 1) throw new Error('Unsupported adapter capability schema');
        if (active) setMatrix(value);
      })
      .catch((e) => {
        if (active) setError(errorText(e));
      });
    return () => {
      active = false;
    };
  }, [open, matrix, attempt]);
  const features = Object.entries(FEATURES) as [Feature, string][];
  return (
    <details
      className="raw-config adapter-capabilities"
      onToggle={(e) => setOpen(e.currentTarget.open)}
    >
      <summary>{t('Provider capabilities')}</summary>
      <p>
        {t(
          'Capabilities integrated in Lumen. This does not check installation, sign-in or model availability.',
        )}
      </p>
      {!desktop && <p>{t('Open the desktop app to view provider capabilities.')}</p>}
      {desktop && !matrix && !error && open && (
        <p role="status">{t('Loading provider capabilities…')}</p>
      )}
      {error && (
        <div role="alert">
          <p>
            {t('Could not load provider capabilities.')} {error}
          </p>
          <button className="text-button" onClick={() => setAttempt((n) => n + 1)}>
            {t('Retry')}
          </button>
        </div>
      )}
      {matrix && (
        <>
          <div
            className="capability-table"
            role="region"
            aria-label={t('Provider capability comparison')}
            tabIndex={0}
          >
            <table>
              <caption>{t('Provider capability comparison')}</caption>
              <thead>
                <tr>
                  <th scope="col">{t('Capability')}</th>
                  {matrix.adapters.map((adapter) => (
                    <th scope="col" key={adapter.provider}>
                      {adapter.label}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {features.map(([key, label]) => (
                  <tr key={key}>
                    <th scope="row">{t(label)}</th>
                    {matrix.adapters.map((adapter) => (
                      <td key={adapter.provider}>{t(STATUS[adapter[key].support])}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p>
            {t(
              'Expand a provider for its limits. Supported features still follow the selected permissions.',
            )}
          </p>
          {matrix.adapters.map((adapter) => (
            <details className="raw-config" key={adapter.provider}>
              <summary>{adapter.label}</summary>
              <dl className="capability-notes">
                {features
                  .filter(([key]) => adapter[key].note)
                  .map(([key, label]) => (
                    <div key={key}>
                      <dt>{t(label)}</dt>
                      <dd>{t(adapter[key].note)}</dd>
                    </div>
                  ))}
              </dl>
            </details>
          ))}
        </>
      )}
    </details>
  );
}
