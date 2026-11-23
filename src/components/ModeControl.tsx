import { Sparkles } from 'lucide-react';
import type { Mode, Provider } from '../lib/types';
import { nameOf } from '../lib/util';
import { useI18n } from '../lib/i18n';

export interface ModeControlProps {
  mode: Mode;
  providers?: Provider[];
  onChange: (mode: Mode) => void;
}

/** Segmented AI provider picker: Auto, Claude, GPT and the other connected providers. */
export default function ModeControl({ mode, providers = [], onChange }: ModeControlProps) {
  const { t } = useI18n();
  const options: Mode[] = ['auto', ...providers];
  return (
    <div className="mode-control" role="group" aria-label={t('AI provider')}>
      {options.map((p) => (
        <button
          key={p}
          aria-pressed={mode === p}
          className={mode === p ? 'active' : ''}
          onClick={() => onChange(p)}
        >
          {p === 'auto' && <Sparkles size={13} />}
          <span>{p === 'auto' ? t('Auto') : nameOf(p)}</span>
        </button>
      ))}
    </div>
  );
}
