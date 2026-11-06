import { useSyncExternalStore } from 'react';
import type { Draft } from './draft';

interface Props {
  draft: Draft;
  label: string;
  /** Receives the (edited) text; the host decides what "use" means. */
  onUse: (text: string) => void;
  useLabel?: string;
  disabled?: boolean;
}

/** Generate on click, edit freely, then explicitly use. Manual typing always works. */
export default function DraftPanel({ draft, label, onUse, useLabel = 'Use', disabled }: Props) {
  const s = useSyncExternalStore(draft.subscribe, draft.getState);
  return (
    <div className="draft-panel">
      <textarea
        aria-label={label}
        value={s.text}
        onChange={(e) => draft.edit(e.target.value)}
        placeholder="Write it yourself or generate a draft"
      />
      {s.error && <p role="alert">{s.error}</p>}
      <div className="draft-actions">
        {s.status === 'generating' ? (
          <button type="button" onClick={() => draft.cancel()}>
            Cancel
          </button>
        ) : (
          <button type="button" disabled={disabled} onClick={() => void draft.generate()}>
            {s.text ? 'Regenerate' : 'Generate draft'}
          </button>
        )}
        <button
          type="button"
          disabled={!s.text.trim() || s.status === 'generating'}
          onClick={() => {
            const t = draft.take();
            if (t) onUse(t);
          }}
        >
          {useLabel}
        </button>
      </div>
    </div>
  );
}
