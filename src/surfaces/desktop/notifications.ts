import { useEffect } from 'react';
import { sessionTitle } from '../../features/sessions/sessions';
import { useT } from '../../i18n';
import { useSyncClient } from '../../sync/react';

const ACTIVE = new Set(['running', 'queued']);
const FAILED = new Set(['failed', 'error', 'interrupted', 'partial']);

/**
 * Tells the user when a conversation finishes, stops or needs them. The shell shows it only while
 * the window is not in front; in front, the conversation already says so.
 */
export function useNotifications() {
  const t = useT();
  const sync = useSyncClient();
  useEffect(() => {
    let last = new Map(sync.getState().sessions.map((x) => [x.id, x.status]));
    const stop = sync.subscribe(() => {
      const sessions = sync.getState().sessions;
      for (const session of sessions) {
        const before = last.get(session.id);
        if (!before || before === session.status) continue;
        const title = sessionTitle(session, t.untitled);
        if (session.status === 'waiting') sync.notify(t.notify.attention, title);
        else if (ACTIVE.has(before) && session.status === 'completed') sync.notify(t.notify.done, title);
        else if (ACTIVE.has(before) && FAILED.has(session.status)) sync.notify(t.notify.failed, title);
      }
      last = new Map(sessions.map((x) => [x.id, x.status]));
    });
    return () => void stop();
  }, [sync, t]);
}
