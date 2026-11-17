import ApiCost from './ApiCost';
import { lazy, memo, Suspense, useState } from 'react';
import {
  BookmarkPlus,
  Check,
  ChevronDown,
  Copy,
  MapPin,
  Paperclip,
  Repeat2,
  RotateCcw,
} from 'lucide-react';
import AttachmentPreview from './AttachmentPreview';
import NativeQuestions from './NativeQuestions';
import QuotaResume from './QuotaResume';
import ChildSessions from './ChildSessions';
import ResponseImage from './ResponseImage';
import { HIDDEN, orchestraOf, providerOf } from '../lib/messages';
import { splitReasoning } from '../lib/reasoning';
import type { Account, ActivityDetail, Message } from '../lib/types';
import { call } from '../lib/bridge';
import { errorText, Mark, nameOf } from '../lib/util';
import { useI18n } from '../lib/i18n';

const MessageBody = lazy(() => import('./MessageBody'));
const OrchestraCard = lazy(() => import('./OrchestraCard'));

/** One transcript entry: author, actions, reasoning, activity, body and lane/Orchestra state.
 * Memoized: a streamed token changes only the live message object, so other rows skip render. */
export default memo(function MessageView({
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
  sessionId,
}: {
  m: Message;
  sessionId?: string;
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
  const { t } = useI18n();
  const [copied, setCopied] = useState(false);
  const visible = m.activity.filter((a) => !HIDDEN.includes(a.kind));
  const inline =
    m.role === 'assistant'
      ? splitReasoning(
          m.text.replace(
            /<lumen-(remember|routine)[\s\S]*?(<\/lumen-(remember|routine)>|$)\n?/g,
            '',
          ),
        )
      : { reasoning: '', answer: m.text };
  const proposed = m.activity.find((a) => a.kind === 'memory-proposed')?.text.split('\n') ?? [];
  const kept = m.activity.filter((a) => a.kind === 'memory-global').map((a) => a.text);
  const recipe = m.activity.find((a) => a.kind === 'recipe-saved')?.text;
  const rememberedAll = m.activity
    .filter((a) => a.kind === 'memory-saved')
    .flatMap((a) => a.text.split('\n'))
    .filter(Boolean);
  const remembered = rememberedAll.length ? rememberedAll : undefined;
  const held = m.activity.find((a) => a.kind === 'memory-held')?.text.split('\n') ?? [];
  const reasoning = [
    ...m.activity.filter((a) => a.kind === 'thinking').map((a) => a.text),
    inline.reasoning,
  ]
    .filter(Boolean)
    .join('\n\n');
  const attachments = m.activity.find((a) => a.kind === 'attachments')?.text.split('\n');
  const messageFolder = m.activity.find((a) => a.kind === 'workspace-root')?.text || workspace;
  const references = m.activity.find((a) => a.kind === 'references')?.text.split('\n');
  const lane = m.activity.find((a) => ['worktree', 'applied', 'discarded'].includes(a.kind));
  const from = providerOf(m);
  const automatic = m.activity.some((a) => a.kind === 'auto-assignment');
  const assigned = m.activity.filter((a) => a.kind === 'model').at(-1)?.text;
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
          {m.delivery && (
            <span role="status">
              {t(m.delivery === 'sending' ? 'Sending…' : 'Delivery not confirmed')}
            </span>
          )}
          {m.role === 'user' && !m.delivery && onRewind && !running && (
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
          {m.role === 'assistant' && m.text && !running && from && !recipe && (
            <button
              className="icon-button"
              aria-label={t('Save as recipe')}
              title={t('Save as recipe: every assistant can repeat this task when you ask')}
              onClick={() => void onOrchestra('save_recipe', { message: m.id })}
            >
              <BookmarkPlus size={13} />
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
      {automatic && assigned && (
        <p className="assignment-note">
          {t('Prompt assigned to')} {assigned}
        </p>
      )}
      {latest && m.role === 'assistant' && (
        <QuotaResume message={m} running={running} busy={busy} command={onOrchestra} />
      )}
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
                {a.detail && sessionId ? (
                  <BoundedOutput
                    session={sessionId}
                    message={m.id}
                    text={a.text}
                    detail={a.detail}
                  />
                ) : (
                  <pre>{a.text}</pre>
                )}
              </details>
            ))}
          </div>
        </details>
      )}
      {m.role === 'assistant' && <ApiCost activity={m.activity} />}
      <NativeQuestions activity={m.activity} live={live} />
      <ChildSessions activity={m.activity} />
      {orchestraState && (
        <Suspense fallback={null}>
          <OrchestraCard
            key={`${m.id}-${orchestraState.stage}`}
            message={m}
            state={orchestraState}
            accounts={accounts.filter(
              (a) =>
                !['opencode', 'cursor', 'openaiapi', 'anthropicapi', 'googleapi'].includes(
                  a.provider,
                ),
            )}
            busy={busy || (running && ['questions', 'plan'].includes(orchestraState.stage))}
            readOnly={readOnly}
            run={onOrchestra}
          />
        </Suspense>
      )}
      {!(orchestraState && ['questions', 'plan'].includes(orchestraState.stage)) && (
        <div className="prose">
          <Suspense fallback={<p>{inline.answer}</p>}>
            <MessageBody text={inline.answer} folder={messageFolder} />
          </Suspense>
        </div>
      )}
      {m.activity.some((a) => a.kind === 'routine-created') && (
        <p className="memory-saved" role="status">
          {t('Recurring task scheduled. Stop it above the message box.')}
        </p>
      )}
      {proposed.map((fact) => (
        <p className="memory-saved" role="status" key={fact}>
          {kept.includes(fact) ? t('Saved for all projects') : t('Remember for all projects?')}:{' '}
          {fact}
          {!kept.includes(fact) && (
            <button
              className="text-button"
              disabled={busy}
              onClick={() => void onOrchestra('remember_global', { message: m.id, fact })}
            >
              {t('Save')}
            </button>
          )}
        </p>
      ))}
      {held
        .filter((fact) => !rememberedAll.includes(fact))
        .map((fact) => (
          <p className="memory-saved" role="status" key={`held-${fact}`}>
            {t('Not saved automatically: this reply read content from outside the project')}.{' '}
            {t('Save to LUMEN.md?')}: {fact}
            <button
              className="text-button"
              disabled={busy}
              onClick={() => void onOrchestra('remember_project', { message: m.id, fact })}
            >
              {t('Save')}
            </button>
          </p>
        ))}
      {recipe && (
        <p className="memory-saved" role="status">
          {t('Saved as recipe')}: {recipe}. {t('Ask any assistant to repeat it.')}
        </p>
      )}
      {remembered && (
        <p className="memory-saved" role="status">
          {t('Remembered in LUMEN.md')}: {remembered.join(' · ')}
        </p>
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
      {m.activity
        .filter((a) => a.kind === 'image')
        .map((a, i) => {
          let path = a.text;
          let folder = messageFolder;
          try {
            const image: unknown = JSON.parse(a.text);
            if (
              image &&
              typeof image === 'object' &&
              'path' in image &&
              typeof image.path === 'string'
            ) {
              path = image.path;
              if ('folder' in image && typeof image.folder === 'string') folder = image.folder;
            }
          } catch {
            /* Native providers may emit a plain image path. */
          }
          return (
            <ResponseImage
              key={i}
              href={path}
              alt={path.split(/[\\/]/).at(-1) || 'Image'}
              folder={folder}
            />
          );
        })}
      {automatic && assigned && !live && m.text && (
        <p className="assignment-note">
          {t('Completed by')} {assigned}
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
    </article>
  );
});

/** Bounded tool output or diff: the full text is fetched only when the reader asks for it. */
function BoundedOutput({
  session,
  message,
  text,
  detail,
}: {
  session: string;
  message: string;
  text: string;
  detail: ActivityDetail;
}) {
  const { t } = useI18n();
  const [full, setFull] = useState<string | null>(null);
  const [state, setState] = useState('');
  const kb = (n: number) => `${Math.ceil(n / 1024)} KB`;
  return (
    <>
      <pre>{full ?? text}</pre>
      {full === null && (
        <p className="bounded-output">
          <small>
            {t('Showing')} {kb(detail.shownBytes)} / {kb(detail.totalBytes)} · {detail.totalLines}{' '}
            {t('lines')}
          </small>{' '}
          <button
            className="text-button"
            disabled={state === 'loading'}
            onClick={() => {
              setState('loading');
              call<string>('activity_detail', { id: session, message, digest: detail.digest })
                .then((value) => {
                  setFull(value);
                  setState('');
                })
                .catch((e) => setState(errorText(e)));
            }}
          >
            {t('Show full output')}
          </button>
          {state && state !== 'loading' && <span role="alert">{state}</span>}
        </p>
      )}
    </>
  );
}
