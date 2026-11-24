import { useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../../i18n';
import type { Session } from '../../protocol/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import { Composer } from '../composer/Composer';
import { MarkdownHostContext, type MarkdownHost } from './Markdown';
import { ConversationContext, type ConversationHost } from './MessageView';
import { Transcript } from './Transcript';
import s from './Conversation.module.css';

const ACTIVE = new Set(['running', 'queued', 'waiting']);

/** A session: its transcript, the user's messages on their way, and the composer. */
export function Conversation({ session }: { session: Session }) {
  const t = useT();
  const sync = useSyncClient();
  const transcript = useSyncState((st) => st.transcripts[session.id]);
  const allOutgoing = useSyncState((st) => st.outgoing);
  const outgoing = useMemo(() => allOutgoing.filter((o) => o.sessionId === session.id), [allOutgoing, session.id]);
  const running = ACTIVE.has(session.status);

  const host = useMemo<ConversationHost>(() => ({ sessionId: session.id, folder: session.workspace, call: (command, args) => sync.call(command, args) }), [sync, session.id, session.workspace]);
  const links = useMemo<MarkdownHost>(
    () => ({
      // The daemon validates the target and the shell opens it; nothing opens from the page.
      openLink: (href) => void sync.call('open_response_link', { folder: session.workspace, href }).catch(() => {}),
      image: (href) => sync.call<string>('response_image_data', { folder: session.workspace, href }),
    }),
    [sync, session.workspace],
  );

  return (
    <ConversationContext value={host}>
      <MarkdownHostContext value={links}>
        <Announcer status={session.status} />
        {!transcript ? (
          <p className={s.placeholder}>{t.loading}</p>
        ) : transcript.messages.length === 0 && outgoing.length === 0 ? (
          <p className={s.placeholder}>{t.emptyTranscript}</p>
        ) : (
          <Transcript messages={transcript.messages} running={running} outgoing={outgoing} onDismiss={(id) => sync.dismiss(id)} />
        )}
        <Composer session={session} />
      </MarkdownHostContext>
    </ConversationContext>
  );
}

/** Says when a turn starts, ends or fails: politely, and never once per streamed token. */
function Announcer({ status }: { status: string }) {
  const t = useT();
  const previous = useRef(status);
  const [message, setMessage] = useState('');
  useEffect(() => {
    const was = previous.current;
    previous.current = status;
    if (was === status) return;
    if (ACTIVE.has(status) && !ACTIVE.has(was)) setMessage(t.conversationText.replying);
    else if (!ACTIVE.has(status) && ACTIVE.has(was)) setMessage(t.status[status] ?? status);
  }, [status, t]);
  return (
    <div className={s.announcer} aria-live="polite">
      {message}
    </div>
  );
}
