import { useCallback, useEffect, useState } from 'react';
import { GitBranch } from 'lucide-react';
import { call, external } from '../lib/bridge';
import type { Published, Session, TaskWorktreeStatus } from '../lib/types';
import { errorText } from '../lib/util';
import { useI18n } from '../lib/i18n';

/** The conversation's task worktree, refreshed whenever the conversation changes or stops. */
export function useTaskWorktree(session: Session | undefined, running: boolean) {
  const [status, setStatus] = useState<TaskWorktreeStatus | null>(null);
  const id = session?.id;
  const workspace = session?.workspace;
  const refresh = useCallback(async () => {
    if (!id) return setStatus(null);
    setStatus(
      await call<TaskWorktreeStatus | null>('task_worktree_status', { id, preview: false }),
    );
  }, [id]);
  useEffect(() => {
    if (running) return;
    refresh().catch(() => setStatus(null));
  }, [refresh, running, workspace]);
  // While Lumen follows a pull request, its state changes without a turn.
  const watching = !!status?.pr?.watching;
  useEffect(() => {
    if (!watching || running) return;
    const timer = setInterval(() => refresh().catch(() => undefined), 60_000);
    return () => clearInterval(timer);
  }, [watching, running, refresh]);
  const [published, setPublished] = useState<Published | null>(null);
  return {
    status,
    refresh,
    published: published?.session.id === id ? published : null,
    setPublished,
  };
}

/** The pull request Lumen just opened; it outlives the worktree it came from. */
export function PublishedNotice({ done, onClose }: { done: Published; onClose: () => void }) {
  const { t } = useI18n();
  return (
    <div className="task-worktree-done" role="status">
      <a
        href={done.url}
        onClick={(e) => {
          e.preventDefault();
          void external(done.url);
        }}
      >
        {done.url}
      </a>
      <span>
        {done.autoMergeError
          ? `${t('Automatic merge is not active:')} ${done.autoMergeError}`
          : t('Merges automatically when CI and review pass.')}
      </span>
      <button className="text-button" onClick={onClose}>
        {t('Close')}
      </button>
    </div>
  );
}

/** Pubblica, Applica alla cartella and Scarta for a conversation that works in its own worktree. */
export default function TaskWorktreeBar({
  session,
  status,
  running,
  onSession,
  onPublished,
  onError,
}: {
  session: Session;
  status: TaskWorktreeStatus;
  running: boolean;
  onSession: (s: Session) => void;
  onPublished: (p: Published) => void;
  onError: (text: string) => void;
}) {
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  const [publishing, setPublishing] = useState<TaskWorktreeStatus | null>(null);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const [base, setBase] = useState('');
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const empty = status.ahead === 0 && !status.dirty;

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      onError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  const openPublish = () =>
    run(async () => {
      const preview = await call<TaskWorktreeStatus | null>('task_worktree_status', {
        id: session.id,
        preview: true,
      });
      if (!preview) return;
      setBase(preview.base);
      setTitle(session.title);
      setBody(t('Prepared with Lumen.'));
      setPublishing(preview);
    });
  const publish = () =>
    run(async () => {
      const result = await call<Published>('publish_task', { id: session.id, base, title, body });
      setPublishing(null);
      onPublished(result);
      onSession(result.session);
    });
  const apply = () =>
    run(async () => onSession(await call<Session>('apply_task', { id: session.id })));
  const stopWatching = () =>
    run(async () => {
      await call('stop_watching_task', { id: session.id });
      onSession(session);
    });
  const discard = () =>
    run(async () => {
      setConfirmDiscard(false);
      onSession(await call<Session>('discard_task', { id: session.id }));
    });

  return (
    <>
      <div className="task-worktree" data-testid="task-worktree">
        <span className="task-worktree-branch" title={status.path}>
          <GitBranch size={13} />
          {status.branch}
          <span className="task-worktree-count">
            {status.ahead === 1 ? t('1 commit') : `${status.ahead} ${t('commits')}`}
            {status.dirty ? ` · ${t('uncommitted changes')}` : ''}
          </span>
        </span>
        {status.pr ? (
          <>
            <a
              className="task-worktree-pr"
              href={status.pr.url}
              onClick={(e) => {
                e.preventDefault();
                void external(status.pr!.url);
              }}
            >
              {t('Pull request')}
            </a>
            <span className="task-worktree-count">
              {t(status.pr.last)}
              {status.pr.rounds > 0 ? ` · ${t('corrections')} ${status.pr.rounds}/3` : ''}
            </span>
            {status.pr.watching && (
              <button disabled={busy} onClick={() => void stopWatching()}>
                {t('Stop following')}
              </button>
            )}
          </>
        ) : (
          <>
            <button
              className="primary"
              disabled={busy || running || empty}
              onClick={() => void openPublish()}
            >
              {t('Publish')}
            </button>
            <button disabled={busy || running || empty} onClick={() => void apply()}>
              {t('Apply to folder')}
            </button>
          </>
        )}
        <button disabled={busy || running} onClick={() => setConfirmDiscard(true)}>
          {t('Discard')}
        </button>
      </div>
      {confirmDiscard && (
        <div className="dialog-scrim">
          <div className="popup conversation-dialog" role="dialog" aria-label={t('Discard')}>
            <p>
              {status.pr ? (
                <>
                  {t('Delete the worktree?')} {t('The branch stays for its pull request.')}
                </>
              ) : (
                <>
                  {t('Delete the worktree and branch')} <code>{status.branch}</code>?{' '}
                  {t('The project folder is not changed.')}
                </>
              )}
            </p>
            <button onClick={() => void discard()} disabled={busy}>
              {t('Discard')}
            </button>
            <button onClick={() => setConfirmDiscard(false)}>{t('Cancel')}</button>
          </div>
        </div>
      )}
      {publishing && (
        <div className="dialog-scrim">
          <div
            className="popup conversation-dialog publish-dialog"
            role="dialog"
            aria-label={t('Publish')}
          >
            <p>
              <strong>{t('Publish as a pull request')}</strong>
            </p>
            <dl>
              <dt>{t('Remote')}</dt>
              <dd>{publishing.remote || '—'}</dd>
              <dt>{t('Branch')}</dt>
              <dd>{publishing.branch}</dd>
            </dl>
            <label>
              {t('Base branch')}
              <input value={base} onChange={(e) => setBase(e.target.value)} />
            </label>
            <label>
              {t('Title')}
              <input value={title} onChange={(e) => setTitle(e.target.value)} />
            </label>
            <label>
              {t('Description')}
              <textarea value={body} rows={4} onChange={(e) => setBody(e.target.value)} />
            </label>
            <p className="publish-note">
              {publishing.blocked ||
                t(
                  'Lumen pushes the branch, opens the pull request and turns on automatic merge. It then follows checks and reviews and corrects failures, up to three times.',
                )}
            </p>
            <button
              className="primary"
              disabled={busy || !!publishing.blocked || !base.trim() || !title.trim()}
              onClick={() => void publish()}
            >
              {busy ? t('Publishing…') : t('Publish')}
            </button>
            <button onClick={() => setPublishing(null)}>{t('Cancel')}</button>
          </div>
        </div>
      )}
    </>
  );
}
