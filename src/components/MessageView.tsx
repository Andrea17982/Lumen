import { lazy, Suspense, useState } from 'react';
import { Check, ChevronDown, Copy, MapPin, Paperclip, Repeat2, RotateCcw } from 'lucide-react';
import AttachmentPreview from './AttachmentPreview';
import WorkingIndicator from './WorkingIndicator';
import { HIDDEN, orchestraOf, providerOf } from '../lib/messages';
import { splitReasoning, workPhase } from '../lib/reasoning';
import type { Account, Message } from '../lib/types';
import { errorText, Mark, nameOf } from '../lib/util';

const MessageBody = lazy(() => import('./MessageBody'));
const OrchestraCard = lazy(() => import('./OrchestraCard'));

/** One transcript entry: author, actions, reasoning, activity, body and lane/Orchestra state. */
export default function MessageView({
  m,
  inLane = false,
  latest,
  live,
  running,
  busy,
  readOnly,
  workspace,
  accounts,
  onRewind,
  onAudit,
  onApplyLane,
  onOrchestra,
  onError,
}: {
  m: Message;
  inLane?: boolean;
  latest: boolean;
  /** The reply that is streaming right now. */
  live: boolean;
  running: boolean;
  busy: boolean;
  readOnly: boolean;
  workspace?: string;
  accounts: Account[];
  /** Absent outside a saved conversation. */
  onRewind?: (m: Message) => void;
  onAudit: (m: Message) => void;
  onApplyLane: (m: Message) => void;
  onOrchestra: (command: string, args: Record<string, unknown>) => Promise<void>;
  onError: (text: string) => void;
}) {
  const [copied, setCopied] = useState(false);
  const visible = m.activity.filter((a) => !HIDDEN.includes(a.kind));
  const inline =
    m.role === 'assistant' ? splitReasoning(m.text) : { reasoning: '', answer: m.text };
  const reasoning = [
    ...m.activity.filter((a) => a.kind === 'thinking').map((a) => a.text),
    inline.reasoning,
  ]
    .filter(Boolean)
    .join('\n\n');
  const attachments = m.activity.find((a) => a.kind === 'attachments')?.text.split('\n');
  const references = m.activity.find((a) => a.kind === 'references')?.text.split('\n');
  const lane = m.activity.find((a) => ['worktree', 'applied', 'discarded'].includes(a.kind));
  const from = providerOf(m);
  const other = from === 'claude' ? 'gpt' : 'claude';
  const orchestraState = m.role === 'assistant' ? orchestraOf(m) : null;
  const nogit = m.activity.some((a) => a.kind === 'orchestra-nogit');
  return (
    <article className={`message ${m.role}${latest ? ' is-latest' : ''}`}>
      <div className="message-author">
        {m.role === 'assistant' ? (
          <>
            <Mark small />
            <span>{inLane && from ? nameOf(from) : 'Lumen'}</span>
          </>
        ) : (
          <span className="user-mark">You</span>
        )}
        <span className="message-actions">
          {m.role === 'user' && onRewind && !running && (
            <button
              className="icon-button"
              aria-label="Rewind to here"
              title="Rewind to here"
              onClick={() => onRewind?.(m)}
            >
              <RotateCcw size={13} />
            </button>
          )}
          {m.role === 'assistant' && m.text && !running && from && (
            <button
              className="text-button audit-button"
              title={`Send to ${nameOf(other)} for audit`}
              onClick={() => onAudit(m)}
            >
              <Repeat2 size={13} />
              Audit with {nameOf(other)}
            </button>
          )}
          {m.role === 'assistant' && m.text && (
            <button
              className="icon-button"
              aria-label="Copy response"
              onClick={() => {
                void navigator.clipboard
                  .writeText(inline.answer)
                  .then(() => {
                    setCopied(true);
                    setTimeout(() => setCopied(false), 1800);
                  })
                  .catch((e) => onError(errorText(e)));
              }}
            >
              {copied ? <Check size={14} /> : <Copy size={14} />}
            </button>
          )}
        </span>
      </div>
      {reasoning && (
        <details className="reasoning">
          <summary>
            <ChevronDown size={13} />
            Reasoning
          </summary>
          <pre>{reasoning}</pre>
        </details>
      )}
      {visible.length > 0 && (
        <details className="activity">
          <summary>
            <span className="activity-indicator" />
            {visible.some((a) => a.kind === 'error')
              ? 'Activity & diagnostics'
              : visible.some((a) => a.kind === 'fallback')
                ? 'Switched provider · activity'
                : `${visible.length} activity ${visible.length === 1 ? 'event' : 'events'}`}
            <ChevronDown size={13} />
          </summary>
          <div className="activity-content">
            {visible.map((a, i) => (
              <details key={i}>
                <summary>
                  {a.kind === 'file'
                    ? 'File changes'
                    : a.kind.charAt(0).toUpperCase() + a.kind.slice(1)}
                </summary>
                <pre>{a.text}</pre>
              </details>
            ))}
          </div>
        </details>
      )}
      {orchestraState && (
        <Suspense fallback={null}>
          <OrchestraCard
            key={`${m.id}-${orchestraState.stage}`}
            message={m}
            state={orchestraState}
            accounts={accounts}
            busy={busy || (running && ['questions', 'plan'].includes(orchestraState.stage))}
            readOnly={readOnly}
            run={onOrchestra}
          />
        </Suspense>
      )}
      {!(orchestraState && ['questions', 'plan'].includes(orchestraState.stage)) && (
        <div className="prose">
          <Suspense fallback={<p>{inline.answer}</p>}>
            <MessageBody text={inline.answer} />
          </Suspense>
        </div>
      )}
      {nogit && !running && (
        <p className="orchestra-closed">
          Orchestra needs git in this folder.{' '}
          <button
            className="text-button"
            onClick={() => void onOrchestra('orchestra_init_git', {})}
          >
            Initialize git
          </button>
        </p>
      )}
      {attachments && (
        <div className="attachment-chips">
          {attachments.map((f) => (
            <span key={f}>
              <Paperclip size={11} /> {f.split(/[\\/]/).filter(Boolean).at(-1) || f}
              {/\.(png|jpe?g|webp|gif)$/i.test(f) && workspace && (
                <AttachmentPreview folder={workspace} path={f} />
              )}
            </span>
          ))}
        </div>
      )}
      {references && (
        <div
          className="attachment-chips"
          aria-label="Repository map locations added to this request"
          title="Added to the prompt from the repository map"
        >
          {references.map((r) => (
            <span key={r}>
              <MapPin size={11} /> {r}
            </span>
          ))}
        </div>
      )}
      {lane?.kind === 'worktree' && !running && (
        <button className="primary-button lane-apply" onClick={() => onApplyLane(m)}>
          Apply {from ? nameOf(from) : 'this'} version
        </button>
      )}
      {lane && lane.kind !== 'worktree' && (
        <small className="lane-state">
          {lane.kind === 'applied' ? 'Applied to your project' : 'Discarded'}
        </small>
      )}
      {live && <WorkingIndicator phase={workPhase(m.activity.map((a) => a.kind))} />}
    </article>
  );
}
