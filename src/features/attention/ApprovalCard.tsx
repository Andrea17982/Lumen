import { useRef, useState } from 'react';
import { Button, TextField } from '../../design/components';
import { Icon } from '../../design/icons';
import { useT } from '../../i18n';
import type { ApprovalAction, ApprovalRequest, ApprovalSession } from '../../protocol/types';
import { Diff } from '../conversation/MessageView';
import { agentName, blocker, countdown, visibleText } from './requests';
import s from './Attention.module.css';

/**
 * One Gatekeeper request. The card shows what the broker recorded and sends back only the
 * request's id, scope and the user's action: never the request's content.
 */
export function ApprovalCard({ request, sessions, now, onDecide }: { request: ApprovalRequest; sessions: ApprovalSession[]; now: number; onDecide: (action: ApprovalAction, note: string | null) => Promise<void> }) {
  const t = useT();
  const [note, setNote] = useState('');
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // The provider names the agent: a worker's own name is agent-chosen text and could read as "Lumen".
  const agent = agentName(request.scope);
  const tool = visibleText(request.tool);
  const title =
    request.kind === 'command' ? t.attention.command(agent) : request.kind === 'fileChange' ? t.attention.fileChange(agent) : request.kind === 'tool' ? t.attention.tool(agent, tool) : t.attention.unsupported(agent);
  const why = blocker(request, sessions, now);
  const open = request.status === 'pending';
  const disabled = working || !!why || !open;
  // One decision per press, even before the next render disables the buttons.
  const sending = useRef(false);

  async function decide(action: ApprovalAction) {
    if (disabled || sending.current || (action === 'approve' && !request.canApprove)) return;
    sending.current = true;
    setWorking(true);
    setError(null);
    try {
      await onDecide(action, note.trim() || null);
    } catch (e) {
      setError(visibleText(String(e)));
    } finally {
      sending.current = false;
      setWorking(false);
    }
  }

  const blockerText = why === 'expired' ? t.attention.expired : why === 'workerGone' ? t.attention.workerGone(visibleText(request.worker?.task ?? '')) : why === 'disconnected' ? t.attention.disconnected : null;
  return (
    <section className={s.card} aria-label={title} data-state={open ? 'open' : 'closed'}>
      <header className={s.head}>
        <Icon name={request.kind === 'fileChange' ? 'file' : 'terminal'} />
        <h3>{title}</h3>
        <span className={s.status} role="status">
          {open ? (why ? '' : t.attention.expiresIn(countdown(request.expiresAtMs - now))) : t.attention.outcome[request.status]}
        </span>
      </header>
      {request.worker && <p className={s.meta}>{t.attention.worker(visibleText(request.worker.task))}</p>}
      {request.command !== null && (
        <pre className={s.command} aria-label={t.kinds.command}>
          {visibleText(request.command)}
        </pre>
      )}
      <p className={s.meta}>
        {t.attention.folder}: <code>{visibleText(request.cwd)}</code>
      </p>
      {request.scope.provider === 'claude' && request.kind === 'command' && <p className={s.meta}>{t.attention.claudeFolder}</p>}
      <p className={s.reason}>{request.reason ? visibleText(request.reason) : t.attention.noReason}</p>
      {request.diffs.map((change, i) => (
        <details key={`${i}:${change.path}`} className={s.diff} open>
          <summary>
            <code>{visibleText(change.path)}</code>
          </summary>
          <Diff text={visibleText(change.diff)} />
        </details>
      ))}
      <details className={s.exact}>
        <summary>{t.attention.exact}</summary>
        <pre>{visibleText(JSON.stringify(request.input, null, 2) ?? '')}</pre>
      </details>
      {!request.canApprove && request.unavailableReason && <p className={s.warning}>{t.attention.cannotApprove(visibleText(request.unavailableReason))}</p>}
      {request.delivery === 'uncertain' && (
        <p className={s.warning} role="alert">
          {t.attention.uncertain}
        </p>
      )}
      {blockerText && open && <p className={s.warning}>{blockerText}</p>}
      {request.note && !open && <p className={s.meta}>{visibleText(request.note)}</p>}
      {open && !why && (
        <>
          <TextField label={t.attention.note} value={note} onChange={setNote} isDisabled={working} />
          <div className={s.actions}>
            {/* Codex's cancel declines and ends the turn; Claude's would only decline, like Reject. */}
            {request.scope.provider === 'codex' && (
              <Button variant="plain" onPress={() => void decide('cancel')} isDisabled={disabled}>
                {t.attention.stopTurn}
              </Button>
            )}
            <Button variant="secondary" onPress={() => void decide('reject')} isDisabled={disabled}>
              {t.reject}
            </Button>
            <Button variant="primary" onPress={() => void decide('approve')} isDisabled={disabled || !request.canApprove}>
              {t.attention.approveOnce}
            </Button>
          </div>
        </>
      )}
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
