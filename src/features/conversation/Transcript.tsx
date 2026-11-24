import { startTransition, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import type { Message } from '../../protocol/types';
import type { Outgoing } from '../../sync/sync';
import { MessageView } from './MessageView';
import s from './Conversation.module.css';

/** M0: the newest messages mount at once, earlier ones in chunks inside a transition. */
const FIRST_MOUNT = 100;
const MOUNT_CHUNK = 250;

/**
 * The whole conversation stays in the DOM (native find, selection and screen readers keep
 * working); `content-visibility` skips layout of what is off screen.
 */
export function Transcript({ messages, running, outgoing, onDismiss }: { messages: Message[]; running: boolean; outgoing: Outgoing[]; onDismiss: (id: string) => void }) {
  const t = useT();
  const [mounted, setMounted] = useState(FIRST_MOUNT);
  const scroller = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);
  const [behind, setBehind] = useState(false);

  useEffect(() => {
    if (mounted >= messages.length) return;
    const id = requestAnimationFrame(() => startTransition(() => setMounted((m) => m + MOUNT_CHUNK)));
    return () => cancelAnimationFrame(id);
  }, [mounted, messages.length]);

  // Follow the reply only while the reader is already at the bottom.
  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el) return;
    if (pinned.current) el.scrollTop = el.scrollHeight;
    else setBehind(true);
  }, [messages, outgoing]);

  const onScroll = () => {
    const el = scroller.current!;
    pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 48;
    if (pinned.current) setBehind(false);
  };

  const shown = messages.slice(Math.max(0, messages.length - mounted));
  const lastAssistant = running ? messages.findLastIndex((m) => m.role === 'assistant') : -1;
  const offset = messages.length - shown.length;
  return (
    <div className={s.conversation}>
      <div ref={scroller} className={s.scroller} onScroll={onScroll}>
        <div className={s.transcript} role="log" aria-label={t.conversation}>
          {shown.map((m, i) => (
            <MessageView key={m.id} message={m} live={offset + i === lastAssistant} />
          ))}
          {outgoing.map((o) => (
            <article key={o.id} className={s.message} data-role="user" aria-label={`${t.you}: ${o.text.slice(0, 120)}`}>
              <h2 id={`author-${o.id}`} className={s.author}>
                {t.you}
                <span className={s.model}>{o.state === 'sending' ? t.composer.sending : o.state === 'failed' ? t.composer.failed : o.state === 'unconfirmed' ? t.composer.unconfirmed : ''}</span>
              </h2>
              <p className={s.userText}>{o.text}</p>
              {o.error && o.state === 'failed' && (
                <p className={s.error}>
                  {o.error}{' '}
                  <button type="button" className={s.inlineButton} onClick={() => onDismiss(o.id)}>
                    {t.composer.dismiss}
                  </button>
                </p>
              )}
            </article>
          ))}
        </div>
      </div>
      {behind && (
        <button
          type="button"
          className={s.jump}
          onClick={() => {
            const el = scroller.current!;
            el.scrollTop = el.scrollHeight;
            pinned.current = true;
            setBehind(false);
          }}
        >
          {t.conversationText.newMessages}
        </button>
      )}
    </div>
  );
}
