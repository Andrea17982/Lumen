import { createContext, memo, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../../i18n';
import type { Catalog } from '../../i18n/en';
import type { Activity, Message } from '../../protocol/types';
import { filesOf, opening, partsOf, stepBody, stepTitle, type MessageParts } from './activity';
import { blocks, StreamingBlocks } from './blocks';
import { CodeBlock, Markdown } from './Markdown';
import { NoGitCard, OrchestraCard } from '../orchestra/Orchestra';
import { orchestraOf } from '../orchestra/state';
import { Notes } from './Notes';
import { RewindButton } from './Rewind';
import s from './Conversation.module.css';

/** What a message needs from its conversation: where it lives and how to ask the daemon. */
export interface ConversationHost {
  sessionId: string;
  folder: string;
  call<T>(command: string, args: Record<string, unknown>): Promise<T>;
}
export const ConversationContext = createContext<ConversationHost | null>(null);
const useHost = () => useContext(ConversationContext)!;

/**
 * One message. Memoized on the message object: the store replaces only the object that changed,
 * so a streamed token re-renders the live reply and nothing else.
 */
export const MessageView = memo(function MessageView({ message, live }: { message: Message; live: boolean }) {
  const t = useT();
  const parts = useMemo(() => partsOf(message), [message]);
  const orchestra = useMemo(() => orchestraOf(message), [message.activity]);
  const host = useHost();
  const user = message.role === 'user';
  return (
    <article className={s.message} data-role={message.role} aria-label={`${user ? t.you : t.appName}: ${opening(parts.answer)}`}>
      <h2 id={`author-${message.id}`} className={s.author}>
        {user ? t.you : t.appName}
        {parts.model && <span className={s.model}>{parts.model}</span>}
      </h2>
      {parts.why && <p className={s.why}>{t.composer.why(parts.why)}</p>}
      {parts.attachments.length > 0 && (
        <ul className={s.attachments} aria-label={t.conversationText.attachments}>
          {parts.attachments.map((a) => (
            <li key={a}>{a}</li>
          ))}
        </ul>
      )}
      {parts.reasoning && (
        <details className={s.reasoning}>
          <summary>{live && !parts.answer ? t.conversationText.thinking : t.conversationText.reasoning}</summary>
          <p>{parts.reasoning}</p>
        </details>
      )}
      {orchestra && <OrchestraCard sessionId={host.sessionId} message={message} state={orchestra} />}
      {message.activity.some((a) => a.kind === 'orchestra-nogit') && <NoGitCard sessionId={host.sessionId} />}
      {parts.steps.length > 0 && <Steps message={message} steps={parts.steps} live={live} />}
      {parts.asked.length > 0 && <Asked asked={parts.asked} />}
      {user ? <p className={s.userText}>{parts.answer}</p> : <Answer text={parts.answer} live={live} />}
      {user && (
        <div className={s.messageActions}>
          <RewindButton messageId={message.id} />
        </div>
      )}
      {!user && !live && parts.answer && !orchestra && <Notes sessionId={host.sessionId} message={message} />}
      {parts.images.map((path) => (
        <WorkspaceImage key={path} path={path} />
      ))}
      {parts.errors.map((e, i) => (
        <p key={i} className={s.error} role="note">
          {e}
        </p>
      ))}
    </article>
  );
});

/** What the agent asked and what it was told. Open questions are answered above the composer. */
function Asked({ asked }: { asked: MessageParts['asked'] }) {
  const t = useT();
  return (
    <dl className={s.asked}>
      {asked.flatMap(({ request, result }) =>
        request.questions.map((q) => {
          const answer = result?.cancelled ? t.attention.closed : result ? (result.answers[q.id]?.join(', ') ?? (q.secret ? t.attention.notSaved : '—')) : '…';
          return (
            <div key={`${request.id}:${q.id}`}>
              <dt>{q.question}</dt>
              <dd>{answer}</dd>
            </div>
          );
        }),
      )}
    </dl>
  );
}

function Answer({ text, live }: { text: string; live: boolean }) {
  const stream = useRef<StreamingBlocks | null>(null);
  if (live && !stream.current) stream.current = new StreamingBlocks();
  const list = live ? stream.current!.update(text) : blocks(text);
  if (!text) return null;
  return <Markdown blocks={list} />;
}

function Steps({ message, steps, live }: { message: Message; steps: Activity[]; live: boolean }) {
  const t = useT();
  const last = steps.at(-1)!;
  // The list mounts on open: thousands of closed rows were most of a long transcript's cost.
  const [open, setOpen] = useState(false);
  return (
    <details className={s.steps} onToggle={(e) => setOpen(e.currentTarget.open)}>
      <summary>
        <span>{t.activities(steps.length)}</span>
        {live && <span className={s.current}>{label(t, last.kind)}: {stepTitle(last).detail}</span>}
      </summary>
      {open && (
        <ol>
          {steps.map((step, i) => (
            <Step key={i} message={message} step={step} />
          ))}
        </ol>
      )}
    </details>
  );
}

function label(t: Catalog, kind: string) {
  return t.kinds[kind] ?? kind;
}

function Step({ message, step }: { message: Message; step: Activity }) {
  const t = useT();
  const title = stepTitle(step);
  // Outputs mount only when opened.
  const [open, setOpen] = useState(false);
  return (
    <li>
      <details className={s.step} onToggle={(e) => setOpen(e.currentTarget.open)}>
        <summary>
          <span className={s.kind}>{label(t, step.kind)}</span>
          <span className={s.stepDetail}>{title.detail}</span>
        </summary>
        {open && (step.kind === 'file' ? <FileChanges step={step} /> : <StepBody message={message} step={step} />)}
      </details>
    </li>
  );
}

/** Bounded output shows what arrived and fetches the rest only on request. */
function StepBody({ message, step }: { message: Message; step: Activity }) {
  const t = useT();
  const host = useHost();
  const [full, setFull] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const text = full ?? stepBody(step);
  return (
    <div className={s.stepBody}>
      <CodeBlock code={text} />
      {step.detail && full === null && (
        <p className={s.truncated}>
          {t.conversationText.truncated(step.detail.totalLines)}
          <button
            type="button"
            onClick={() =>
              host.call<string>('activity_detail', { id: host.sessionId, message: message.id, digest: step.detail!.digest }).then(setFull, (e) =>
                setError(String(e).includes('no longer available') ? t.conversationText.detailGone : String(e)),
              )
            }
          >
            {t.conversationText.showAll}
          </button>
        </p>
      )}
      {error && <p className={s.error}>{error}</p>}
    </div>
  );
}

function FileChanges({ step }: { step: Activity }) {
  const files = filesOf(step);
  if (!files.length) return <CodeBlock code={step.text} />;
  return (
    <ul className={s.files}>
      {files.map((f) => (
        <FileChange key={f.path} path={f.path} kind={f.kind} />
      ))}
    </ul>
  );
}

function FileChange({ path, kind }: { path: string; kind?: string }) {
  const t = useT();
  const host = useHost();
  const [diff, setDiff] = useState<{ text: string; truncated: boolean } | null>(null);
  const [error, setError] = useState<string | null>(null);
  return (
    <li>
      <button type="button" className={s.fileButton} aria-expanded={!!diff} onClick={() => (diff ? setDiff(null) : host.call<{ text: string; truncated: boolean }>('file_change_diff', { folder: host.folder, file: path }).then(setDiff, (e) => setError(String(e))))}>
        <code>{path}</code>
        {kind && <span className={s.kind}>{kind}</span>}
      </button>
      {diff && <Diff text={diff.text} />}
      {diff?.truncated && <p className={s.truncated}>{t.conversationText.diffTruncated}</p>}
      {error && <p className={s.error}>{error}</p>}
    </li>
  );
}

export function Diff({ text }: { text: string }) {
  return (
    <pre className={s.diff} tabIndex={0}>
      {text.split('\n').map((line, i) => (
        <span key={i} data-line={line.startsWith('+') && !line.startsWith('+++') ? 'add' : line.startsWith('-') && !line.startsWith('---') ? 'remove' : line.startsWith('@@') ? 'hunk' : undefined}>
          {line + '\n'}
        </span>
      ))}
    </pre>
  );
}

function WorkspaceImage({ path }: { path: string }) {
  const host = useHost();
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    // response_image_data keeps the path inside the project; attachment_image_data would read any
    // absolute path, and image paths come from agent output.
    host.call<string>('response_image_data', { folder: host.folder, href: path }).then(
      (url) => live && url.startsWith('data:image/') && setSrc(url),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [host, path]);
  return src ? <img className={s.image} src={src} alt={path} /> : null;
}
