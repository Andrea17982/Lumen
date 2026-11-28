import { useEffect, useState } from 'react';
import { Button, ConfirmDialog } from '../../design/components';
import { useT } from '../../i18n';
import { isSessionActive, type Session } from '../../protocol/types';
import { useSyncClient } from '../../sync/react';
import { visibleText } from '../attention/requests';
import s from './Worktree.module.css';

interface WorktreeStatus {
  branch: string;
  path: string;
  ahead: number;
  base: string;
  upstream: string;
  blocked: string | null;
  pr: { url: string; watching: boolean; rounds: number; last: string } | null;
}

export function WorktreeBar({ session }: { session: Session }) {
  const t = useT();
  const sync = useSyncClient();
  const [status, setStatus] = useState<WorktreeStatus | null>(null);
  const [discard, setDiscard] = useState(false);
  const [publishing, setPublishing] = useState(false);
  const [configure, setConfigure] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busy = isSessionActive(session.status);
  const load = () => sync.call<WorktreeStatus | null>('task_worktree_status', { id: session.id, preview: false }).then(setStatus, (e) => setError(visibleText(String(e))));
  // Read again whenever the conversation changes state: a turn may have started or ended the branch.
  useEffect(() => {
    void load();
  }, [sync, session.id, session.status, session.workspace]);
  if (!status) return error && <p className={s.error} role="alert">{error}</p>;
  const undo = () => {
    setError(null);
    sync.mutate('discard_task', { id: session.id }).then(
      () => setStatus(null),
      (e) => setError(visibleText(String(e))),
    );
  };
  async function publish() {
    setPublishing(true);
    setConfigure(null);
    setError(null);
    try {
      const preview = await sync.call<WorktreeStatus | null>('task_worktree_status', { id: session.id, preview: true });
      if (!preview) return setStatus(null);
      if (preview.blocked) {
        setConfigure(visibleText(preview.blocked));
        return;
      }
      await sync.mutate('publish_task', { id: session.id, base: preview.base, title: session.title.trim() || preview.upstream, body: '' });
      await load();
    } catch (e) {
      setError(visibleText(String(e)));
    } finally {
      setPublishing(false);
    }
  }
  return (
    <div className={s.bar} role="region" aria-label={t.worktree.label} data-tour-target="task">
      <span className={s.branch}>
        {t.worktree.on}
        <code>{visibleText(status.branch)}</code>
      </span>
      <span className={s.quiet}>{t.worktree.commits(status.ahead)}</span>
      <span className={s.spacer} />
      {status.pr ? (
        <PullRequest session={session} status={status} onChange={load} onError={(e) => setError(visibleText(String(e)))} />
      ) : (
        <Button variant="secondary" isDisabled={busy || publishing} onPress={() => void publish()}>
          {t.worktree.publish}
        </Button>
      )}
      <Button variant="plain" isDisabled={busy || publishing} onPress={() => setDiscard(true)}>
        {t.worktree.discard}
      </Button>
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
      {configure && <p className={s.error} role="alert">{t.worktree.configure} {configure}</p>}
      <ConfirmDialog isOpen={discard} onOpenChange={setDiscard} title={t.worktree.discardTitle} body={t.worktree.discardBody(visibleText(status.path))} confirmLabel={t.worktree.discard} cancelLabel={t.cancel} onConfirm={undo} />
    </div>
  );
}

function PullRequest({ session, status, onChange, onError }: { session: Session; status: WorktreeStatus; onChange: () => void; onError: (e: unknown) => void }) {
  const t = useT();
  const sync = useSyncClient();
  const pr = status.pr!;
  return (
    <span className={s.pr}>
      <button type="button" className={s.link} onClick={() => void sync.call('open_response_link', { folder: session.workspace, href: pr.url }).catch(onError)}>
        {t.worktree.pr}
      </button>
      <span className={s.quiet} title={pr.watching ? t.worktree.watching : undefined}>
        {[visibleText(pr.last), pr.rounds ? t.worktree.rounds(pr.rounds) : ''].filter(Boolean).join(' · ')}
      </span>
      {pr.watching && (
        <Button variant="plain" isDisabled={isSessionActive(session.status)} onPress={() => void sync.call('stop_watching_task', { id: session.id }).then(onChange, onError)}>
          {t.worktree.stopWatching}
        </Button>
      )}
    </span>
  );
}
