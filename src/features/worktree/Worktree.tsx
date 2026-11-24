import { useEffect, useState } from 'react';
import { Badge, Button, ConfirmDialog, TextField } from '../../design/components';
import { useT } from '../../i18n';
import type { Session } from '../../protocol/types';
import { useSyncClient } from '../../sync/react';
import { visibleText } from '../attention/requests';
import s from './Worktree.module.css';

export interface WorktreeStatus {
  branch: string;
  origin: string;
  path: string;
  ahead: number;
  dirty: boolean;
  remote: string | null;
  base: string;
  upstream: string;
  continues: string | null;
  blocked: string | null;
  pr: { url: string; watching: boolean; rounds: number; last: string } | null;
}
const BUSY = new Set(['running', 'queued', 'waiting']);

/**
 * A large task runs on its own branch in its own worktree. This bar says so and offers the three
 * ways out: publish it as a pull request, apply it to the folder, or discard it.
 */
export function WorktreeBar({ session }: { session: Session }) {
  const t = useT();
  const sync = useSyncClient();
  const [status, setStatus] = useState<WorktreeStatus | null>(null);
  const [dialog, setDialog] = useState<'publish' | 'apply' | 'discard' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busy = BUSY.has(session.status);
  const load = () => sync.call<WorktreeStatus | null>('task_worktree_status', { id: session.id, preview: false }).then(setStatus, () => setStatus(null));
  // Read again whenever the conversation changes state: a turn may have started or ended the branch.
  useEffect(() => {
    void load();
  }, [sync, session.id, session.status, session.workspace]);
  if (!status) return null;
  const finish = (command: string) => {
    setError(null);
    sync.mutate(command, { id: session.id }).then(
      () => setStatus(null),
      (e) => setError(visibleText(String(e))),
    );
  };
  return (
    <div className={s.bar} role="region" aria-label={t.worktree.label}>
      <span className={s.branch}>
        {t.worktree.on}
        <code>{visibleText(status.branch)}</code>
      </span>
      <span className={s.quiet}>{t.worktree.commits(status.ahead)}</span>
      {status.dirty && <Badge tone="warning">{t.worktree.uncommitted}</Badge>}
      <span className={s.spacer} />
      {status.pr ? (
        <PullRequest session={session} status={status} onChange={load} />
      ) : (
        <>
          <Button variant="primary" isDisabled={busy} onPress={() => setDialog('publish')}>
            {t.worktree.publish}
          </Button>
          <Button variant="secondary" isDisabled={busy} onPress={() => setDialog('apply')}>
            {t.worktree.apply}
          </Button>
        </>
      )}
      <Button variant="plain" isDisabled={busy} onPress={() => setDialog('discard')}>
        {t.worktree.discard}
      </Button>
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
      {dialog === 'publish' && (
        <Publish
          session={session}
          onClose={() => setDialog(null)}
          onPublish={(args) => {
            setError(null);
            sync.mutate('publish_task', { id: session.id, ...args }).then(load, (e) => setError(visibleText(String(e))));
          }}
        />
      )}
      <ConfirmDialog isOpen={dialog === 'apply'} onOpenChange={(o) => !o && setDialog(null)} title={t.worktree.applyTitle} body={t.worktree.applyBody} confirmLabel={t.worktree.apply} cancelLabel={t.cancel} onConfirm={() => finish('apply_task')} />
      <ConfirmDialog isOpen={dialog === 'discard'} onOpenChange={(o) => !o && setDialog(null)} title={t.worktree.discardTitle} body={t.worktree.discardBody} confirmLabel={t.worktree.discard} cancelLabel={t.cancel} danger onConfirm={() => finish('discard_task')} />
    </div>
  );
}

function PullRequest({ session, status, onChange }: { session: Session; status: WorktreeStatus; onChange: () => void }) {
  const t = useT();
  const sync = useSyncClient();
  const pr = status.pr!;
  return (
    <span className={s.pr}>
      <button type="button" className={s.link} onClick={() => void sync.call('open_response_link', { folder: session.workspace, href: pr.url }).catch(() => {})}>
        {t.worktree.pr}
      </button>
      <span className={s.quiet} title={pr.watching ? t.worktree.watching : undefined}>
        {[visibleText(pr.last), pr.rounds ? t.worktree.rounds(pr.rounds) : ''].filter(Boolean).join(' · ')}
      </span>
      {pr.watching && (
        <Button variant="plain" onPress={() => void sync.call('stop_watching_task', { id: session.id }).then(onChange, () => {})}>
          {t.worktree.stopWatching}
        </Button>
      )}
    </span>
  );
}

/** Publish shows what it will push and where before doing it; the daemon says why when it cannot. */
function Publish({ session, onClose, onPublish }: { session: Session; onClose: () => void; onPublish: (args: { base: string; title: string; body: string }) => void }) {
  const t = useT();
  const sync = useSyncClient();
  const [preview, setPreview] = useState<WorktreeStatus | null>(null);
  const [base, setBase] = useState('');
  const [title, setTitle] = useState(session.title);
  const [body, setBody] = useState('');
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    sync.call<WorktreeStatus | null>('task_worktree_status', { id: session.id, preview: true }).then(
      (p) => {
        setPreview(p);
        setBase(p?.base ?? '');
      },
      (e) => setError(visibleText(String(e))),
    );
  }, [sync, session.id]);
  return (
    <ConfirmDialog
      isOpen
      onOpenChange={(o) => !o && onClose()}
      title={t.worktree.publishTitle}
      body={
        <div className={s.fields}>
          {preview && (
            <dl className={s.facts}>
              <div>
                <dt>{t.worktree.remote}</dt>
                <dd>{visibleText(preview.remote ?? '—')}</dd>
              </div>
              <div>
                <dt>{t.worktree.branch}</dt>
                <dd>
                  <code>{visibleText(preview.upstream || preview.branch)}</code>
                </dd>
              </div>
            </dl>
          )}
          {preview?.continues && <p className={s.quiet}>{t.worktree.continues(visibleText(preview.continues))}</p>}
          <TextField label={t.worktree.base} value={base} onChange={setBase} />
          <TextField label={t.worktree.title} value={title} onChange={setTitle} />
          <TextField label={t.worktree.body} value={body} onChange={setBody} multiline />
          {preview?.blocked && (
            <p className={s.error} role="alert">
              {visibleText(preview.blocked)}
            </p>
          )}
          {error && (
            <p className={s.error} role="alert">
              {error}
            </p>
          )}
        </div>
      }
      confirmLabel={t.worktree.publishNow}
      cancelLabel={t.cancel}
      onConfirm={() => onPublish({ base: base.trim(), title: title.trim(), body })}
    />
  );
}
