import { AlertCircle, Sparkles, X } from 'lucide-react';
import type { Plan } from '../lib/types';
import { IconButton, nameOf } from '../lib/util';

/** A dismissible error line, used by the composer and the Settings panel. */
export function ErrorBanner({
  text,
  label,
  onDismiss,
}: {
  text: string;
  /** Accessible name of the dismiss button. */
  label: string;
  onDismiss: () => void;
}) {
  return (
    <div role="alert" className="error-banner">
      <AlertCircle size={16} />
      <span>{text}</span>
      <IconButton label={label} onClick={onDismiss}>
        <X size={14} />
      </IconButton>
    </div>
  );
}

/** Files attached to the next message, each removable. */
export function AttachmentList({
  paths,
  onRemove,
}: {
  paths: string[];
  onRemove: (path: string) => void;
}) {
  if (!paths.length) return null;
  return (
    <div className="attachment-list" aria-label="Attached files">
      {paths.map((path) => {
        const name = path.split(/[\\/]/).filter(Boolean).at(-1) || path;
        return (
          <span className="attachment-chip" key={path} title={path}>
            <span>{name}</span>
            <button type="button" aria-label={`Remove ${name}`} onClick={() => onRemove(path)}>
              <X size={12} />
            </button>
          </span>
        );
      })}
    </div>
  );
}

const STRATEGY: Record<string, string> = {
  single: 'Direct',
  broadcast: 'Claude and ChatGPT side by side',
  orchestra: 'Orchestra',
};

/** How Auto planned the last turn: provider, strategy and reasoning effort. */
export function PlanLine({ plan }: { plan: Plan }) {
  return (
    <div className="plan-line">
      <Sparkles size={13} />
      <span>
        {nameOf(plan.provider)} · {STRATEGY[plan.strategy] ?? 'Answer + cross-audit'} ·{' '}
        {plan.effort} reasoning
      </span>
    </div>
  );
}
