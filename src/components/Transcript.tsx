import { lazy, memo, Suspense, useMemo } from 'react';
import type { ComponentProps, RefObject } from 'react';
import MessageView from './MessageView';
import { laneOf } from '../lib/messages';
import type { Message } from '../lib/types';
import { virtualTranscriptExperiment } from '../lib/transcriptExperiment';

const VirtualWindow = virtualTranscriptExperiment
  ? lazy(() => import('../../tests/fixtures/virtualization/VirtualWindow'))
  : null;

const ManualMerge = lazy(() => import('./ManualMerge'));
type Props = Omit<ComponentProps<typeof MessageView>, 'm' | 'latest' | 'live' | 'inLane'> & {
  messages: Message[];
  lastMessageId?: string;
  sessionId?: string;
  bottom: RefObject<HTMLDivElement | null>;
  onMerged: () => Promise<void>;
  onDiscardLane: (message: Message) => void;
};

/** Full accessible transcript; composer edits do not rebuild historical message rows. */
function Transcript({
  messages,
  lastMessageId,
  sessionId,
  bottom,
  onMerged,
  onDiscardLane,
  ...props
}: Props) {
  const groups = useMemo(() => {
    const result: (Message | Message[])[] = [];
    for (const message of messages) {
      const last = result.at(-1);
      if (laneOf(message)) {
        if (Array.isArray(last)) last.push(message);
        else result.push([message]);
      } else result.push(message);
    }
    return result;
  }, [messages]);
  const view = (m: Message, inLane = false) => (
    <MessageView
      key={m.id}
      {...props}
      m={m}
      inLane={inLane}
      latest={m.id === messages.at(-1)?.id}
      live={props.running && m.role === 'assistant' && m.id === lastMessageId}
    />
  );
  const content = groups.map((g) => {
    if (!Array.isArray(g)) return view(g);
    const worktree = g.find((m) => m.activity.some((a) => a.kind === 'worktree'));
    return (
      <div className="lanes" key={g[0].id}>
        <div className="lane-grid">{g.map((m) => view(m, true))}</div>
        {sessionId && !props.running && worktree && (
          <Suspense fallback={null}>
            <ManualMerge session={sessionId} message={worktree.id} onApplied={onMerged} />
          </Suspense>
        )}
        {worktree && !props.running && (
          <button className="text-button lane-discard" onClick={() => onDiscardLane(worktree)}>
            Discard both versions
          </button>
        )}
      </div>
    );
  });
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
