import type { ReactNode } from 'react';
import { X } from 'lucide-react';
import { DraftPanel, type Draft } from '../features/local-assistant';

interface Props {
  title: string;
  draft: Draft;
  label: string;
  useLabel: string;
  onUse: (text: string) => void;
  onClose: () => void;
  /** Extra actions below the editor, e.g. a fallback that does not need the draft. */
  footer?: ReactNode;
}

/** A reviewable local draft shown above the composer. Nothing leaves it until the user uses it. */
export default function DraftSheet({
  title,
  draft,
  label,
  useLabel,
  onUse,
  onClose,
  footer,
}: Props) {
  return (
    <section className="draft-sheet" aria-label={title}>
      <header>
        <strong>{title}</strong>
        <button
          className="icon-button"
          aria-label="Close draft"
          onClick={() => {
            draft.cancel();
            onClose();
          }}
        >
          <X size={14} />
        </button>
      </header>
      <DraftPanel draft={draft} label={label} onUse={onUse} useLabel={useLabel} />
      {footer}
    </section>
  );
}
