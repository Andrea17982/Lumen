import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { Button } from '../../design/components';
import { Icon } from '../../design/icons';
import { useLocale, useT } from '../../i18n';
import { formatDate, formatTime } from '../../i18n/format';
import type { Locale } from '../../i18n';
import { isSessionActive, type Session } from '../../protocol/types';
import { useSyncClient, useSyncState } from '../../sync/react';
import { openQuestions, recovery, shown, type Recovery } from './requests';
import { elsewhereLabel } from './quota';
import { ApprovalCard } from './ApprovalCard';
import { QuestionCard } from './QuestionCard';
import s from './Attention.module.css';

const EMPTY: never[] = [];

/**
 * What the agent is waiting for in this session: Gatekeeper approvals and native questions.
 * Sits above the composer; nothing here decides anything without the user's action.
 */
export function Attention({ session }: { session: Session }) {
  const t = useT();
  const sync = useSyncClient();
  const approvals = useSyncState((st) => st.approvals[session.id]);
  const pending = useSyncState((st) => st.questions);
  const messages = useSyncState((st) => st.transcripts[session.id]?.messages) ?? EMPTY;
  const questions = useMemo(() => openQuestions(messages, pending), [messages, pending]);
  const requests = approvals?.requests ?? EMPTY;
  const stopped = useMemo(() => recovery(messages, session.status), [messages, session.status]);

  // Requests seen open here stay as an outcome line until the turn ends.
  const seen = useRef(new Set<string>());
  const active = isSessionActive(session.status);
  for (const r of requests) if (r.status === 'pending') seen.current.add(r.id);
  useEffect(() => {
    if (!active) seen.current.clear();
  }, [active]);
  const visible = shown(requests, active ? seen.current : new Set(), session.id);

  // The countdown is the only timer, and only while something is open.
  const [now, setNow] = useState(Date.now);
  const counting = visible.some((r) => r.status === 'pending');
  useEffect(() => {
    if (!counting) return;
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [counting]);

  if (!approvals?.error && visible.length === 0 && questions.length === 0 && !stopped) return null;
  return (
    <section className={s.attention} aria-label={t.attention.label}>
      {approvals?.error && (
        <p className={s.warning} role="alert">
          {t.attention.journal(approvals.error)}
        </p>
      )}
      {visible.map((request) => (
        <ApprovalCard
          key={request.id}
          request={request}
          sessions={approvals?.sessions ?? EMPTY}
          now={now}
          onDecide={(action, note) => sync.decide({ requestId: request.id, scope: request.scope, action, note })}
        />
      ))}
      {questions.map((q) => (
        <QuestionCard
          key={q.request.id}
          request={q.request}
          state={q.state}
          onAnswer={(answers) => sync.answer(session.id, q.request.id, answers)}
          onOpenSignIn={() => sync.call('open_connector_url', { id: q.request.id }, session.id)}
        />
      ))}
      {stopped && <RecoveryCard recovery={stopped} session={session} />}
    </section>
  );
}

function when(locale: Locale, ms: number) {
  const today = new Date().toDateString() === new Date(ms).toDateString();
  return today ? formatTime(locale, ms) : `${formatDate(locale, ms)} ${formatTime(locale, ms)}`;
}

/** After a reply stopped: resume once at the quota reset on request, or say nothing continued. */
function RecoveryCard({ recovery, session }: { recovery: Recovery; session: Session }) {
  const t = useT();
  const locale = useLocale();
  const sync = useSyncClient();
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const blocked = useSyncState((st) => st.unconfirmed.some((u) => u.sessionId === session.id));
  const help = useId();
  const run =(command: string, args: Record<string, unknown>) => {
    setWorking(true);
    setError(null);
    sync
      .call(command, args)
      .catch((e) => setError(String(e)))
      .finally(() => setWorking(false));
  };
  return (
    <div className={s.card}>
      {recovery.kind === 'interrupted' ? (
        <>
          <p className={s.reason}>{t.recovery.interrupted}</p>
          <p className={s.meta}>{t.recovery.interruptedHelp}</p>
        </>
      ) : recovery.kind === 'scheduled' ? (
        <div className={s.head}>
          <Icon name="info" />
          <p className={s.reason}>{t.recovery.quotaScheduled(when(locale, recovery.at))}</p>
          <Button variant="secondary" isDisabled={working} onPress={() => run('cancel_run', { id: session.id })}>
            {t.recovery.cancelScheduled}
          </Button>
        </div>
      ) : (
        <>
          <div className={s.actions}>
            {recovery.resume && (
              <Button variant="primary" aria-describedby={`${help}-resume`} isDisabled={working || blocked} onPress={() => run('resume_at_reset', { id: session.id, message: recovery.messageId })}>
                {(recovery.resume.estimated ? t.recovery.quotaResetEstimated : t.recovery.quotaReset)(when(locale, recovery.resume.at))}
              </Button>
            )}
            {recovery.elsewhere && (
              <Button variant={recovery.resume ? 'secondary' : 'primary'} aria-describedby={`${help}-elsewhere`} isDisabled={working || blocked} onPress={() => run('continue_elsewhere', { id: session.id, message: recovery.messageId })}>
                {elsewhereLabel(t, recovery.elsewhere)}
              </Button>
            )}
          </div>
          {recovery.resume && (
            <p id={`${help}-resume`} className={s.meta}>
              {t.recovery.quotaHelp}
            </p>
          )}
          {recovery.elsewhere && (
            <p id={`${help}-elsewhere`} className={s.meta}>
              {recovery.elsewhere.weaker ? `${t.recovery.elsewhereHelp} ${t.recovery.elsewhereWeaker}` : t.recovery.elsewhereHelp}
            </p>
          )}
        </>
      )}
      {error && (
        <p className={s.error} role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
