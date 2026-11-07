import { Circle, Shrink, X } from 'lucide-react';
import { useState } from 'react';
import { createPortal } from 'react-dom';
import type { ContextUse } from '../lib/context';
import { IconButton } from '../lib/util';

const tokens = (n: number) => n.toLocaleString();

function Part({ label, value }: { label: string; value: string }) {
  return (
    <div className="context-part">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

/**
 * Share of the model's context the next prompt would use: a coloured dot beside Usage, and a
 * popover with the exact token count, its parts and a Compact action.
 */
export default function ContextMeter({
  use,
  disabled,
  onCompact,
}: {
  use: ContextUse | null;
  disabled: boolean;
  onCompact: () => void;
}) {
  const [position, setPosition] = useState<{ left: number; bottom: number } | null>(null);
  const percent = use ? Math.round(use.percent) : 0;
  const summary = use
    ? `Context ${percent}%: ${tokens(use.used)} of ${tokens(use.window)} tokens`
    : 'Context size unavailable: the model for this chat is not known yet.';
  return (
    <>
      <button
        className={`usage-button context-meter ${use?.tone ?? 'unknown'}`}
        title={summary}
        aria-label={summary}
        aria-expanded={!!position}
        onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          setPosition((open) =>
            open
              ? null
              : {
                  left: Math.max(12, Math.min(r.right + 10, window.innerWidth - 352)),
                  bottom: Math.max(12, window.innerHeight - r.bottom),
                },
          );
        }}
      >
        <Circle size={8} fill="currentColor" className="context-dot" />
        Context {use ? `${percent}%` : 'n/a'}
      </button>
      {position &&
        // Outside the sidebar, whose transform would clip a fixed popover.
        createPortal(
          <>
            <button
              className="usage-scrim"
              aria-label="Close context"
              onClick={() => setPosition(null)}
            />
            <section
              className="usage-popover popup context-popover"
              role="dialog"
              aria-label="Context window"
              style={position}
              onKeyDown={(e) => {
                if (e.key === 'Escape') setPosition(null);
              }}
            >
              <header>
                <strong>Context</strong>
                <IconButton
                  className="icon-button"
                  label="Close context"
                  onClick={() => setPosition(null)}
                >
                  <X size={14} />
                </IconButton>
              </header>
              {use ? (
                <>
                  <div className={`context-total ${use.tone}`}>
                    <strong>
                      {tokens(use.used)} / {tokens(use.window)}
                    </strong>
                    <span>tokens · {percent}%</span>
                  </div>
                  <div
                    className={`usage-track context-track ${use.tone}`}
                    role="progressbar"
                    aria-label="Context used"
                    aria-valuemin={0}
                    aria-valuemax={use.window}
                    aria-valuenow={use.used}
                  >
                    <i style={{ width: `${use.percent}%` }} />
                  </div>
                  <Part label="Chat, instructions and notes" value={`~${tokens(use.prompt)}`} />
                  <Part label="Draft text (files excluded)" value={`~${tokens(use.draft)}`} />
                  <Part
                    label="CLI instructions and tools"
                    value={use.overhead === null ? 'Not measured' : tokens(use.overhead)}
                  />
                  <p>
                    {use.source === 'measured'
                      ? 'Window and CLI overhead as Claude reported them on its last reply; chat and draft text are counted locally. Attached files and images are excluded.'
                      : 'Local estimate: the provider has not reported its own figures for this chat yet, so the window is the published size for the model. Attached files and images are excluded.'}{' '}
                    Compact when the dot turns orange or red.
                  </p>
                </>
              ) : (
                <p>{summary}</p>
              )}
              <button
                className="text-button context-compact"
                disabled={disabled}
                onClick={() => {
                  setPosition(null);
                  onCompact();
                }}
              >
                <Shrink size={13} /> Compact into a fresh session
              </button>
            </section>
          </>,
          document.body,
        )}
    </>
  );
}
