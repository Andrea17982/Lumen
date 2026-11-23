import { lazy, memo, Suspense } from 'react';
import type { ComponentProps, RefObject } from 'react';
import MessageView from './MessageView';
import type { Message } from '../lib/types';
import { virtualTranscriptExperiment } from '../lib/transcriptExperiment';

const VirtualWindow = virtualTranscriptExperiment
  ? lazy(() => import('../../tests/fixtures/virtualization/VirtualWindow'))
  : null;

type Props = Omit<ComponentProps<typeof MessageView>, 'm' | 'latest' | 'live'> & {
  messages: Message[];
  lastMessageId?: string;
  sessionId?: string;
  bottom: RefObject<HTMLDivElement | null>;
};

/** Full accessible transcript; composer edits do not rebuild historical message rows. */
function Transcript({ messages, lastMessageId, sessionId, bottom, ...props }: Props) {
  const content = messages.map((m) => (
    <MessageView
      key={m.id}
      {...props}
      sessionId={sessionId}
      m={m}
      latest={m.id === messages.at(-1)?.id}
      live={props.running && m.role === 'assistant' && m.id === lastMessageId}
    />
  ));
  return (
    <div className="message-list">
      {VirtualWindow ? (
        <Suspense fallback={null}>
          <VirtualWindow>{content}</VirtualWindow>
        </Suspense>
      ) : (
        content
      )}
      <div ref={bottom} />
    </div>
  );
}
export default memo(Transcript);
