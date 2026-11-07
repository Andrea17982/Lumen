import { Columns2, Sparkles } from 'lucide-react';
import type { Mode } from '../lib/types';
import { nameOf } from '../lib/util';
import { useI18n } from '../lib/i18n';

export interface ModeControlProps {
  mode: Mode;
  /** Read-only sessions can also compare both providers side by side. */
  bothAvailable: boolean;
  onChange: (mode: Mode) => void;
}

/** Segmented AI provider picker: Auto, Claude, GPT (and Both in read-only mode). */
export default function ModeControl({ mode, bothAvailable, onChange }: ModeControlProps) {
  const { t } = useI18n();
  const options: Mode[] = bothAvailable
    ? ['auto', 'claude', 'gpt', 'both']
    : ['auto', 'claude', 'gpt'];
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
          {p === 'both' && <Columns2 size={13} />}
          <span>{p === 'auto' ? t('Auto') : p === 'both' ? t('Both') : nameOf(p)}</span>
        </button>
      ))}
    </div>
  );
}
